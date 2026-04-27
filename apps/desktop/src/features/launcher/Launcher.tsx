import { LauncherArgumentPanel } from "./components/LauncherArgumentPanel";
import { LauncherFooter } from "./components/LauncherFooter";
import { LauncherHeader } from "./components/LauncherHeader";
import { LauncherResultsList } from "./components/LauncherResultsList";
import { LauncherSearchInput } from "./components/LauncherSearchInput";
import { LauncherShell } from "./components/LauncherShell";
import { useLauncherController } from "./useLauncherController";

export function Launcher() {
  const controller = useLauncherController();

  return (
    <LauncherShell shellRef={controller.shellRef}>
      {controller.showHeader ? <LauncherHeader {...controller.header} /> : null}

      <LauncherSearchInput
        inputRef={controller.inputRef}
        mode={controller.inputMode}
        value={controller.query}
        placeholder={controller.placeholder}
        onChange={controller.onQueryChange}
        onKeyDown={controller.onKeyDown}
      />

      {controller.argumentPanel ? <LauncherArgumentPanel {...controller.argumentPanel} /> : null}

      {controller.showResults ? (
        <LauncherResultsList
          items={controller.resultItems}
          showInteractiveSkeleton={controller.showInteractiveSkeleton}
          emptyMessage={controller.emptyMessage}
          onSelect={controller.onResultSelect}
          setItemRef={controller.setResultItemRef}
        />
      ) : null}

      {controller.showFooter ? <LauncherFooter {...controller.footer} /> : null}
    </LauncherShell>
  );
}
