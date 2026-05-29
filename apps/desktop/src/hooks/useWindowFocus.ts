import { useEffect, useState } from "react";

/**
 * Returns whether the browser/app window is currently focused.
 * Used by the sync poller to reduce polling frequency when backgrounded.
 */
export function useWindowFocus(): boolean {
  const [focused, setFocused] = useState(() =>
    typeof document !== "undefined" ? document.hasFocus() : true,
  );

  useEffect(() => {
    const onFocus = () => setFocused(true);
    const onBlur = () => setFocused(false);

    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);

    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, []);

  return focused;
}
