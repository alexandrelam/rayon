import type { KeyboardEventHandler, Ref, RefObject } from "react";

import { Input } from "@/components/ui/input";
import type { LauncherInputMode } from "@/features/launcher/types";

export function LauncherSearchInput({
  inputRef,
  mode,
  onChange,
  onKeyDown,
  placeholder,
  value,
}: {
  inputRef: RefObject<HTMLInputElement | null>;
  mode: LauncherInputMode;
  onChange: (value: string) => void;
  onKeyDown: KeyboardEventHandler<HTMLInputElement>;
  placeholder: string;
  value: string;
}) {
  return (
    <Input
      ref={inputRef as Ref<HTMLInputElement>}
      data-mode={mode}
      value={value}
      onChange={(event) => {
        onChange(event.currentTarget.value);
      }}
      onKeyDown={onKeyDown}
      placeholder={placeholder}
      className={
        mode === "browser_tabs"
          ? "border-[rgba(37,99,235,0.42)] bg-[rgba(234,244,255,0.9)] focus-visible:border-[rgba(37,99,235,0.72)] focus-visible:bg-[rgba(239,247,255,0.98)] focus-visible:ring-[rgba(37,99,235,0.16)]"
          : undefined
      }
      spellCheck={false}
      autoCapitalize="off"
      autoCorrect="off"
      aria-label="Command search"
    />
  );
}
