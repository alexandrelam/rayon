import type { KeyboardEventHandler, RefObject } from "react";

import type { PendingExecution } from "@/commandExecution";

export type LauncherResultItemViewModel = {
  id: string;
  title: string;
  meta: string;
  kind: string;
  selected: boolean;
};

export type LauncherHeaderViewModel = {
  title: string;
  subtitle: string | null;
  version: string | null;
};

export type LauncherFooterViewModel = {
  message: string | null;
  muted: boolean;
  error: string;
};

export type LauncherArgumentPanelViewModel = {
  currentStep: number;
  totalSteps: number;
  flagLabel: string;
  defaultValue: string | null;
};

export type LauncherController = {
  shellRef: RefObject<HTMLElement | null>;
  inputRef: RefObject<HTMLInputElement | null>;
  query: string;
  placeholder: string;
  onQueryChange: (nextQuery: string) => void;
  onKeyDown: KeyboardEventHandler<HTMLInputElement>;
  header: LauncherHeaderViewModel;
  showHeader: boolean;
  showResults: boolean;
  showFooter: boolean;
  resultItems: LauncherResultItemViewModel[];
  showInteractiveSkeleton: boolean;
  emptyMessage: string | null;
  footer: LauncherFooterViewModel;
  argumentPanel: LauncherArgumentPanelViewModel | null;
  onResultSelect: (itemId: string) => void;
  setResultItemRef: (itemId: string, node: HTMLButtonElement | null) => void;
  pendingExecution: PendingExecution | null;
};
