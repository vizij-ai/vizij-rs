import { useEffect, useState } from 'react'
import reactLogo from './assets/react.svg'
import viteLogo from '/vite.svg'
import './App.css'
import wasminit, { WasmAnimationEngine } from 'animation-player'

function App() {
  const [count, setCount] = useState(0)
  const [text, setText] = useState('')
  const [wasmReady, setWasmReady] = useState(false)
  const [wasmError, setWasmError] = useState('')

  useEffect(() => {
    // Initialize the WebAssembly module
    wasminit().then(() => {
      setWasmReady(true)
      setWasmError('')
    }).catch((error) => {
      setWasmReady(false)
      setWasmError(`Failed to load WebAssembly module: ${error.message}`)
    })
  }, [])

  return (
    <>
    <div>
        <a href="https://vite.dev" target="_blank">
          <img src={viteLogo} className="logo" alt="Vite logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <h1>Vite + React</h1>
      <div className="card">
        <button onClick={() => {
          setCount((count) => count + 1)
          new WasmAnimationEngine(null);
          setText("WasmAnimationEngine created successfully!")
        }}>
          Count is {count}. {wasmReady ? text : 'Loading...'}
          {wasmError && <div className="error">{wasmError}</div>}
        </button>
        <p>
          Edit <code>src/App.tsx</code> and save to test HMR
        </p>
      </div>
      <p className="read-the-docs">
        Click on the Vite and React logos to learn more
      </p>
    </>
  )
}

export default App
