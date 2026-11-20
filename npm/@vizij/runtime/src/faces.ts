/**
 * What the page's handle knows of each instance, kept from the module's
 * pushed events: which face each shows, which have a load under way, where
 * each face's safe area lies, and the promise of every load until its answer
 * arrives. Free of the wasm module, so it runs anywhere the module's events
 * can be fed to it.
 */

/** A rectangle of the canvas, in CSS pixels from its top-left corner. */
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** One element of a face, as its `loaded` event describes it. */
export interface FaceElement {
  /** The id its RobotData declares: what picks, the selection and
   * `setValue` name it by. */
  id: string;
  /** What its author called it, `""` where unnamed. */
  name: string;
}

/** A face showing in an instance, as its `loaded` event described it. */
export interface ShownFace {
  instance: string;
  key?: string;
  elements: FaceElement[];
}

/** Something the page's App reports, by its `type`. */
export type VizijEvent =
  /**
   * A Vizij loaded whole into its instance: its scene is indexed, its
   * device built. It draws in its place once `ready`; the face it replaces
   * draws until then. `load`'s promise resolves from this.
   */
  | ({ type: "loaded" } & ShownFace)
  /**
   * A load into an instance that did not finish, and why: its scene failed
   * to load, a later load into the same instance replaced it, or the
   * instance was unloaded. Whatever the instance showed before still shows.
   * `load`'s promise rejects from this.
   */
  | { type: "loadFailed"; instance: string; key?: string; error: string }
  /**
   * A loaded face draws in its place from this frame: every mesh of it in
   * its camera's view has its pipeline compiled and the GPU has finished a
   * frame drawing them. `drawn` is false when readiness gave up waiting to
   * see that, after ten seconds of frames, and made the face ready as it
   * draws. Its device's time starts at this frame.
   */
  | { type: "ready"; instance: string; drawn: boolean }
  /**
   * A click on a face, by its instance: the element under it, or `null` for
   * a click that hit no element. A click is the primary button's or a
   * touch's, decided on release: press and release on the same element
   * pick it, and a release over no element within 2 px of its press is a
   * miss; a drag that ends over nothing is not reported. The element is
   * found over its mesh as drawn, morphs applied; where faces overlap, the
   * one drawn last is clicked.
   */
  | { type: "pick"; instance: string; element: string | null }
  /**
   * Where a face's safe area — the bounds its camera frames — lies on the
   * canvas now, in CSS pixels from its top-left corner, or `null` once it
   * has none (placed on an empty rectangle, or gone). Sent when it moves.
   */
  | { type: "safeArea"; instance: string; area: Rect | null };

/** What takes a face down in the module, and frees a device. */
export interface FaceModule<D> {
  stop(instance: string): void;
  /** Frees a device no page will be handed. */
  dispose(device: D): void;
}

/**
 * A load waiting for its answer. One whose instance was unloaded is already
 * settled, and stays only to take its own answer when that arrives.
 */
interface Pending<D> {
  key?: string;
  device: D;
  resolve: (device: D) => void;
  reject: (error: Error) => void;
  unloaded?: true;
}

/** What the handle knows of one instance. */
interface Instance<D> {
  /** The face showing, and the device its load resolved to, once one is. */
  shown?: { face: ShownFace; device: D };
  /** Loads asked for and not yet answered, oldest first. */
  pending: Pending<D>[];
  /** The load asked for last and not yet answered, so a repeat joins it. */
  loading?: { key?: string; done: Promise<D> };
}

export interface Faces<D> {
  /** Every event the module pushes goes through this, so each load is
   * settled whether or not the page listens; `handler` hears it after. */
  dispatch(handler?: (event: VizijEvent) => void): (event: VizijEvent) => void;
  /** `begin` starts the load in the module and returns its device at once,
   * or throws when the module refuses it outright, with nothing queued. */
  load(instance: string, key: string | undefined, begin: () => D): Promise<D>;
  unload(instance: string): void;
  readonly faces: ReadonlyMap<string, ShownFace>;
  readonly loading: ReadonlySet<string>;
  readonly safeAreas: ReadonlyMap<string, Rect>;
}

/**
 * The instances of one page. An instance's answers arrive in the order its
 * loads were asked for, so each settles the oldest still waiting.
 */
export function trackFaces<D>(module: FaceModule<D>): Faces<D> {
  const instances = new Map<string, Instance<D>>();
  const faces = new Map<string, ShownFace>();
  const safeAreas = new Map<string, Rect>();
  const instance = (id: string): Instance<D> => {
    let held = instances.get(id);
    if (!held) {
      held = { pending: [] };
      instances.set(id, held);
    }
    return held;
  };
  return {
    faces,
    safeAreas,
    get loading() {
      const ids = new Set<string>();
      for (const [id, held] of instances) if (held.loading) ids.add(id);
      return ids;
    },
    dispatch: (handler) => (event) => {
      if (event.type === "loaded" || event.type === "loadFailed") {
        const held = instance(event.instance);
        const settled = held.pending.shift();
        if (settled?.unloaded) {
          // Answered at its unload, its device freed then.
        } else if (event.type === "loaded") {
          const { type: _, ...face } = event;
          // Set before the handler hears of it, so the handle already says
          // which face is showing when it does.
          if (settled) held.shown = { face, device: settled.device };
          faces.set(event.instance, face);
          settled?.resolve(settled.device);
        } else if (settled) {
          // A device whose face never showed is no page's.
          module.dispose(settled.device);
          settled.reject(new Error(event.error));
        }
        // With nothing it asked for left unanswered, the instance is no
        // longer loading — said before the handler hears of it.
        if (!held.pending.some((waiting) => !waiting.unloaded)) held.loading = undefined;
      } else if (event.type === "safeArea") {
        if (event.area) safeAreas.set(event.instance, event.area);
        else safeAreas.delete(event.instance);
      }
      handler?.(event);
    },
    load(id, key, begin) {
      const held = instance(id);
      // Compared with the face last asked for, not only the one showing: a
      // request for the face showing while another loads must replace that
      // one, or the face asked for last would not be the one left showing.
      if (key !== undefined) {
        if (held.loading) {
          if (held.loading.key === key) return held.loading.done;
        } else if (held.shown && held.shown.face.key === key) {
          return Promise.resolve(held.shown.device);
        }
      }
      let device: D;
      try {
        device = begin();
      } catch (error) {
        return Promise.reject(error);
      }
      const answered = new Promise<D>((resolve, reject) =>
        held.pending.push({ key, device, resolve, reject }),
      );
      const loading = {
        key,
        done: answered.finally(() => {
          if (held.loading === loading) held.loading = undefined;
        }),
      };
      held.loading = loading;
      return loading.done;
    },
    unload(id) {
      faces.delete(id);
      module.stop(id);
      const held = instances.get(id);
      if (!held) return;
      held.shown = undefined;
      held.loading = undefined;
      // Anything still loading into it is settled now; the module answers
      // each with a `loadFailed` later. Each stays queued, settled, to take
      // that answer, so a load asked for after this one settles on its own.
      for (const waiting of held.pending) {
        if (waiting.unloaded) continue;
        module.dispose(waiting.device);
        waiting.reject(new Error("its instance was unloaded"));
        waiting.unloaded = true;
      }
    },
  };
}
