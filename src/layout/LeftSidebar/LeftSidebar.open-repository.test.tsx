import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const menuMocks = vi.hoisted(() => ({
  newSessionItems: vi.fn(
    (
      _projectId: string,
      _groupId: string | null,
      _parentSessionId: string | null,
      opts?: { withTerminal?: boolean; withBrowser?: boolean },
    ) => (opts?.withTerminal ? [{ label: "tree.newTerminalSession", onClick: vi.fn() }] : []),
  ),
}));

const storeState = vi.hoisted(() => ({
  leftWidth: 280,
  importProject: vi.fn(),
  addVirtualProject: vi.fn(),
  setCloneModalOpen: vi.fn(),
  renameNode: vi.fn(),
  openSession: vi.fn(),
  treeFilter: "",
  setTreeFilter: vi.fn(),
  selection: [],
  clearSelection: vi.fn(),
  archiveMany: vi.fn(),
  archiveGroup: vi.fn(),
  clearNodeWorktree: vi.fn(),
  globalSearchOpen: false,
  setGlobalSearchOpen: vi.fn(),
  notifications: {},
  pendingSpawns: [] as unknown[],
  clearAllNotifications: vi.fn(),
  clearAllBadges: vi.fn(),
  ephemeralSessions: {},
  browserTabs: {},
  docTabs: {},
  renameScratch: vi.fn(),
  sessions: [],
  runtimes: {},
  statusFilter: null as ("working" | "asking" | "waiting")[] | null,
  dynamicStatusFilter: true,
  setStatusFilter: vi.fn(),
  appendSidebarTreeViewStatusMatches: vi.fn(),
  refreshSidebarTreeViewStatusMatches: vi.fn(),
  setSidebarTreeViewStatusFilter: vi.fn(),
  markFilter: null,
  setMarkFilter: vi.fn(),
  setSidebarTreeViewMarkFilter: vi.fn(),
  splitSidebarTreeView: vi.fn(),
  deleteSidebarTreeView: vi.fn(),
  setActiveSidebarTreeView: vi.fn(),
  resizeSidebarTreeSplit: vi.fn(),
  groups: [{ id: "group-1", projectId: "project-1", worktreePath: null }],
  projects: [{ id: "project-1", rootPath: "/tmp/project" }],
  openBrowserTab: vi.fn(),
  treeMutationError: null as string | null,
}));

const gitMocks = vi.hoisted(() => ({
  isRepo: true as boolean | undefined,
  gitRepoWebUrl: vi.fn<(cwd: string) => Promise<string | null>>(),
}));
const envState = vi.hoisted(() => ({ isTauri: true, isElectron: false }));

vi.mock("../../i18n", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../i18n")>()),
  useT: () => (key: string) => key,
}));
vi.mock("../../store/termStore", () => {
  const useTermStore = Object.assign(
    (selector: (state: typeof storeState) => unknown) => selector(storeState),
    {
      getState: () => storeState,
      setState: (patch: Partial<typeof storeState>) => Object.assign(storeState, patch),
    },
  );
  return { useTermStore };
});
vi.mock("../../platform", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../platform")>();
  return { ...actual, env: new Proxy(actual.env, { get: (target, key) => key in envState ? envState[key as keyof typeof envState] : Reflect.get(target, key) }) };
});
vi.mock("../../ipc/commands", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../ipc/commands")>()),
  gitRepoWebUrl: gitMocks.gitRepoWebUrl,
}));
vi.mock("../../hooks/useGitBranch", () => ({
  isWorktreeGone: () => false,
  peekGitBranchInfo: () => (gitMocks.isRepo === undefined ? undefined : { isRepo: gitMocks.isRepo }),
  prefetchGitBranchInfo: vi.fn(),
}));
vi.mock("../../components/Icons", () => ({
  default: new Proxy({}, { get: () => () => null }),
}));
vi.mock("../../components/ContextMenu", () => ({
  ContextMenu: ({ items }: { items: { label: string; onClick?: () => void }[] }) => (
    <div>
      {items.map((item, index) => (
        <button type="button" key={`${item.label}-${index}`} onClick={() => item.onClick?.()}>
          {item.label}
        </button>
      ))}
    </div>
  ),
}));
vi.mock("../sessionMenu", () => ({
  useSessionMenu: () => ({
    newSessionItems: menuMocks.newSessionItems,
    buildSessionItems: vi.fn(() => []),
    buildScratchItems: vi.fn(() => []),
    buildMoveToMany: vi.fn(() => null),
    buildGitItems: vi.fn(() => []),
    buildMarkItem: vi.fn((_kind: string, _id: string) => ({ label: "mark.menu", submenu: [] })),
    openDialog: vi.fn(),
    dialogs: null,
  }),
}));
vi.mock("./ProjectTree", () => ({
  ProjectTree: ({ onContext }: { onContext: (node: unknown, x: number, y: number) => void }) => (
    <button
      type="button"
      onContextMenu={(event) => {
        event.preventDefault();
        onContext({ kind: "project", id: "project-1", name: "Project", projectId: "project-1" }, 0, 0);
      }}
    >
      Project
    </button>
  ),
}));
vi.mock("../GlobalSearch/GlobalSearch", () => ({ GlobalSearch: () => null }));

import { LeftSidebar } from "./LeftSidebar";

const openProjectMenu = () => {
  render(<LeftSidebar />);
  fireEvent.contextMenu(screen.getByRole("button", { name: "Project" }));
};

describe("LeftSidebar Open Repository", () => {
  beforeEach(() => {
    gitMocks.isRepo = true;
    gitMocks.gitRepoWebUrl.mockReset();
    storeState.openBrowserTab.mockClear();
    storeState.treeMutationError = null;
    envState.isTauri = true;
    envState.isElectron = false;
  });

  it("opens the origin web page of the project in a browser tab", async () => {
    gitMocks.gitRepoWebUrl.mockResolvedValue("https://github.com/owner/repo");
    openProjectMenu();

    fireEvent.click(screen.getByRole("button", { name: "tree.openRepository" }));

    await waitFor(() =>
      expect(storeState.openBrowserTab).toHaveBeenCalledWith("https://github.com/owner/repo"),
    );
    expect(gitMocks.gitRepoWebUrl).toHaveBeenCalledWith("/tmp/project");
    expect(storeState.treeMutationError).toBeNull();
  });

  it("reports a repository without a network origin instead of opening a tab", async () => {
    gitMocks.gitRepoWebUrl.mockResolvedValue(null);
    openProjectMenu();

    fireEvent.click(screen.getByRole("button", { name: "tree.openRepository" }));

    await waitFor(() => expect(storeState.treeMutationError).toBe("tree.openRepositoryNoRemote"));
    expect(storeState.openBrowserTab).not.toHaveBeenCalled();
  });

  it("is hidden for folders that are not Git repositories or not yet probed", () => {
    gitMocks.isRepo = false;
    openProjectMenu();
    expect(screen.queryByRole("button", { name: "tree.openRepository" })).toBeNull();
    cleanup();

    gitMocks.isRepo = undefined;
    openProjectMenu();
    expect(screen.queryByRole("button", { name: "tree.openRepository" })).toBeNull();
  });

  it("is hidden in clients without native browser tabs", () => {
    envState.isTauri = false;
    openProjectMenu();
    expect(screen.queryByRole("button", { name: "tree.openRepository" })).toBeNull();
  });
});
