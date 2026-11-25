/**
 * Controls a driver adds to faces by keys of its own, and what the page hears
 * of them: the shapes `@semio/scene` uses for robots, so one driver can send
 * the same commands to either. A control on a face names it by its instance.
 */

/** A box on screen, in CSS pixels of the canvas, with its centre at `x`, `y`. */
export interface ScreenBox {
  x: number;
  y: number;
  min_x: number;
  min_y: number;
  max_x: number;
  max_y: number;
}

/** One element's box on screen. */
export interface ElementAnchor extends ScreenBox {
  instance: string;
  /** The element's id, as the face's RobotData declares it. */
  element: string;
  /** The name of the element's glTF node. */
  node: string;
}

export type GizmoKind = "Transform" | "Joint";

/** One control's box on screen, around everything it draws. */
export interface GizmoAnchor {
  /** The key the driver gave the control. */
  key: string;
  kind: GizmoKind;
  screen: ScreenBox;
}

/** A transform control's handle, by axis (0, 1, 2 for X, Y, Z), or a joint's. */
export type GizmoHandle =
  { Arrow: number } | { Plane: number } | { Ring: number } | { Ball: number } | "Joint";

/** One amount a control reads, along or about one axis. */
export interface Reading {
  /** 0, 1 or 2 for X, Y or Z; `null` for a joint, whose axis is its own. */
  axis: number | null;
  amount: number;
  unit: "Metres" | "Degrees" | "Factor";
}

/** The control the pointer has. */
export interface GizmoInHand {
  key: string;
  kind: GizmoKind;
  handle: GizmoHandle;
  /** Held in a drag, rather than only under the pointer. */
  held: boolean;
  readings: Reading[];
}

/** What a page needs to draw labels and tooltips over its faces. */
export interface OverlayFrame {
  elements: ElementAnchor[];
  gizmos: GizmoAnchor[];
  pointer: GizmoInHand | null;
}

/**
 * A pose as a driver sees it: Z up, in metres, with the rotation as a
 * quaternion `[x, y, z, w]`. A face's world is Z up already.
 */
export interface Pose {
  position: [number, number, number];
  /** No turn when absent. */
  rotation?: [number, number, number, number];
}

/** What a transform control moves. */
export type ControlTarget =
  /** A face as a whole, by its instance. */
  | { kind: "instance"; instance: string }
  /** One element of a face, by the id its RobotData gives it. */
  | { kind: "element"; instance: string; id: string }
  /**
   * A goal at `pose`, with nothing drawn but the control. `view` names the
   * face it is used through, by its instance: every face has a camera of its
   * own, so a goal naming none would have no view to be used through, and
   * `control` throws for it.
   */
  | { kind: "goal"; pose: Pose; view: string };

/**
 * How big a control is: the same on screen however the face is framed, in
 * CSS pixels, or fixed in the face's world, in its units. A face's controls
 * are 80 pixels unless given a size.
 */
export type ControlSize = { pixels: number } | { metres: number };

/**
 * A control a driver adds. A face has no joints, so `control` throws for a
 * joint control on one.
 */
export type Control =
  | {
      kind: "transform";
      target: ControlTarget;
      size?: ControlSize;
      /** Every mode when absent. */
      modes?: ("translate" | "rotate" | "scale")[];
      /** Handles along the target's own axes rather than the world's. */
      local?: boolean;
      /** The X, Y and Z handles' CSS hex colours; red, green, blue when absent. */
      colors?: [string, string, string];
      /**
       * Turns each arrow, with its square and ball, to the side of its axis
       * facing the camera. Absent, they stay on the positive side, so each
       * arrow shows which way its axis runs.
       */
      faceViewer?: boolean;
    }
  | { kind: "joint"; instance: string; joint: string; color?: string };

/**
 * An instruction about one control, by the driver's key for it, which is
 * unique among every face's controls.
 */
export type ControlCommand =
  | { op: "add"; key: string; control: Control }
  | { op: "remove"; key: string }
  | { op: "show"; key: string; visible: boolean }
  /** Moves a transform control's target to `pose`, in the world. */
  | {
      op: "setPose";
      key: string;
      pose: Pose;
      /** The target's scale along its own axes; kept as it is when absent. */
      scale?: [number, number, number];
    }
  /**
   * Changes a control in place: its `colors`, `local` and `faceViewer`, its `opacity`
   * (faded to over a fifth of a second; how it looks only) and whether it is
   * `interactive`, and its `size`. What is absent stays as it is.
   */
  | {
      op: "configure";
      key: string;
      color?: string;
      colors?: [string, string, string];
      local?: boolean;
      faceViewer?: boolean;
      opacity?: number;
      interactive?: boolean;
      size?: ControlSize;
    }
  /**
   * Draws a control as the one under the pointer is drawn, or stops. `handle`
   * names one handle, as the overlay names it; the whole control when absent.
   */
  | { op: "highlight"; key: string; handle?: GizmoHandle; on: boolean };

/**
 * A drag on a control: what it asks its target to become. **Nothing has
 * moved**: the driver sets the pose if it wants the change.
 */
export interface ControlEvent {
  key: string;
  phase: "start" | "move" | "end";
  /** A transform control's target's pose, in the world. */
  pose?: Required<Pose>;
  /** A transform control's target's scale, along its own axes. */
  scale?: [number, number, number];
}

/**
 * A press with a button other than the primary on a control's handle, the
 * driver's to act on: it moves nothing. `position` is in the canvas's CSS
 * pixels.
 */
export interface ControlPress {
  key: string;
  handle: GizmoHandle;
  button: "secondary" | "middle";
  position: [number, number];
}
