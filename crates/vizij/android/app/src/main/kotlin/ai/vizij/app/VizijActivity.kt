package ai.vizij.app

import android.app.NativeActivity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import java.io.File

/**
 * The app's one activity: the framework's NativeActivity running the vizij
 * library (`libvizij.so`), plus what native code cannot do through a
 * NativeActivity — take a face (a GLB file) from the system's document
 * picker or from a VIEW intent, and hold the screen on with the system bars
 * hidden.
 *
 * A face received is copied to `filesDir/model.glb`. The library loads that
 * file at launch, so the face last opened is the one a relaunch — after a
 * reboot too — shows; while the library runs, [modelOpened] tells it the file
 * changed.
 *
 * The library's settings pane keeps clear of the system bars and the
 * display's cutout, which [insetsChanged] reports, and a back gesture the
 * pane does not take leaves the app ([leaveApp]).
 */
class VizijActivity : NativeActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        // The face a launching VIEW intent carries is in place before
        // super.onCreate starts the library, which reads it at launch.
        receive(intent)
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            window.attributes.layoutInDisplayCutoutMode =
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        hideSystemBars()
        window.decorView.setOnApplyWindowInsetsListener { view, insets ->
            val safe = safeInsets(insets)
            insetsChanged(safe[0], safe[1], safe[2], safe[3])
            view.onApplyWindowInsets(insets)
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        Thread { if (receive(intent)) modelOpened() }.start()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        // The bars a swipe revealed hide again once the face has the focus.
        if (hasFocus) hideSystemBars()
    }

    /** Open the system's document picker; the library calls this on a long press. */
    @Suppress("unused")
    fun openPicker() {
        runOnUiThread {
            val pick = Intent(Intent.ACTION_GET_CONTENT)
                .addCategory(Intent.CATEGORY_OPENABLE)
                // GLB files reach the picker under whatever type their
                // provider guesses (often application/octet-stream), so
                // every type is offered; the library refuses a file that is
                // no face.
                .setType("*/*")
            @Suppress("DEPRECATION")
            startActivityForResult(pick, PICK_FACE)
        }
    }

    /** Move the app to the back, as the system's back does; the library calls this on a back gesture it does not take. */
    @Suppress("unused")
    fun leaveApp() {
        runOnUiThread { moveTaskToBack(true) }
    }

    /**
     * Where the screen's edges are covered — the system bars, shown or
     * hidden (a swipe shows them over the app), and the display's cutout —
     * as left, top, right, bottom, in pixels.
     */
    private fun safeInsets(insets: WindowInsets): IntArray = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.R -> {
            val covered = insets.getInsetsIgnoringVisibility(
                WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout(),
            )
            intArrayOf(covered.left, covered.top, covered.right, covered.bottom)
        }
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.P -> insets.displayCutout?.let {
            intArrayOf(it.safeInsetLeft, it.safeInsetTop, it.safeInsetRight, it.safeInsetBottom)
        } ?: IntArray(4)
        else -> IntArray(4)
    }

    @Deprecated("The AndroidX result API is not available to a NativeActivity.")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        @Suppress("DEPRECATION")
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data
        if (requestCode == PICK_FACE && resultCode == RESULT_OK && uri != null) {
            Thread { if (copy(uri)) modelOpened() }.start()
        }
    }

    /** Copy the face a VIEW intent carries; false for any other intent. */
    private fun receive(intent: Intent?): Boolean {
        if (intent?.action != Intent.ACTION_VIEW) return false
        val uri = intent.data ?: return false
        return copy(uri)
    }

    /**
     * Copy the document at [uri] to `filesDir/model.glb`, through a partial
     * file renamed into place, so a copy cut short never replaces the face.
     */
    private fun copy(uri: Uri): Boolean = try {
        val partial = File(filesDir, "$MODEL.part")
        val input = contentResolver.openInputStream(uri)
            ?: throw IllegalStateException("no stream")
        input.use { source -> partial.outputStream().use { source.copyTo(it) } }
        if (!partial.renameTo(File(filesDir, MODEL))) {
            throw IllegalStateException("cannot move it into place")
        }
        Log.i(TAG, "received a face from $uri")
        true
    } catch (e: Exception) {
        Log.e(TAG, "cannot receive a face from $uri", e)
        false
    }

    private fun hideSystemBars() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            window.insetsController?.let {
                it.hide(WindowInsets.Type.systemBars())
                it.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        } else {
            @Suppress("DEPRECATION")
            window.decorView.systemUiVisibility = (View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                or View.SYSTEM_UI_FLAG_FULLSCREEN
                or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                or View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                or View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION)
        }
    }

    /** The library's: `filesDir/model.glb` changed, load it. */
    private external fun modelOpened()

    /** The library's: the edges [safeInsets] reports changed. */
    private external fun insetsChanged(left: Int, top: Int, right: Int, bottom: Int)

    companion object {
        private const val TAG = "vizij"
        private const val MODEL = "model.glb"
        private const val PICK_FACE = 1

        init {
            // NativeActivity loads the library for its own entry point; this
            // load is the one that binds [modelOpened].
            System.loadLibrary("vizij")
        }
    }
}
