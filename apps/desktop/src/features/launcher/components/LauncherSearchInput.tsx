import type { KeyboardEventHandler, Ref, RefObject } from "react";

import { Input } from "@/components/ui/input";

export function LauncherSearchInput({
  inputRef,
  onChange,
  onKeyDown,
  placeholder,
  value,
}: {
  inputRef: RefObject<HTMLInputElement | null>;
  onChange: (value: string) => void;
  onKeyDown: KeyboardEventHandler<HTMLInputElement>;
  placeholder: string;
  value: string;
}) {
  return (
    <Input
      ref={inputRef as Ref<HTMLInputElement>}
      value={value}
      onChange={(event) => {
        onChange(event.currentTarget.value);
      }}
      onKeyDown={onKeyDown}
      placeholder={placeholder}
      spellCheck={false}
      autoCapitalize="off"
      autoCorrect="off"
      aria-label="Command search"
    />
  );
}
