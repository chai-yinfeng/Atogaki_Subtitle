export type PlaybackAction =
  | "toggle-playback"
  | "rewind"
  | "forward"
  | "previous-subtitle"
  | "next-subtitle"
  | "slower"
  | "faster"
  | "toggle-overlay";

export function playbackActionForKey(event: KeyboardEvent): PlaybackAction | null {
  const code = event.code;
  const key = event.key.toLowerCase();
  if (code === "Space" || code === "KeyK" || key === " " || key === "k") return "toggle-playback";
  if (code === "ArrowLeft" || code === "KeyJ" || key === "arrowleft" || key === "j") return "rewind";
  if (code === "ArrowRight" || code === "KeyL" || key === "arrowright" || key === "l") return "forward";
  if (code === "BracketLeft" || key === "[") return "previous-subtitle";
  if (code === "BracketRight" || key === "]") return "next-subtitle";
  if (code === "Comma" || key === ",") return "slower";
  if (code === "Period" || key === ".") return "faster";
  if (code === "KeyO" || key === "o") return "toggle-overlay";
  return null;
}
