import type { VmState } from "../types";

export type VmAction = "start" | "stop" | "pause" | "delete";

interface VmActionsProps {
  vmId: string;
  state: VmState;
  onAction: (vmId: string, action: VmAction) => void;
  loading?: boolean;
}

export default function VmActions({
  vmId,
  state,
  onAction,
  loading = false,
}: VmActionsProps) {
  const canStart =
    state === "created" || state === "stopped" || state === "paused";
  const canStop = state === "running" || state === "paused";
  const canPause = state === "running";

  return (
    <div className="flex items-center space-x-2">
      {canStart && (
        <button
          className="px-3 py-1.5 text-sm font-medium text-white bg-sky-600 hover:bg-sky-700 rounded-lg transition-colors disabled:opacity-50"
          disabled={loading}
          onClick={() => onAction(vmId, "start")}
        >
          {state === "paused" ? "Resume" : "Start"}
        </button>
      )}
      {canPause && (
        <button
          className="px-3 py-1.5 text-sm font-medium text-gray-700 bg-gray-200 hover:bg-gray-300 rounded-lg transition-colors disabled:opacity-50"
          disabled={loading}
          onClick={() => onAction(vmId, "pause")}
        >
          Pause
        </button>
      )}
      {canStop && (
        <button
          className="px-3 py-1.5 text-sm font-medium text-gray-700 bg-gray-200 hover:bg-gray-300 rounded-lg transition-colors disabled:opacity-50"
          disabled={loading}
          onClick={() => onAction(vmId, "stop")}
        >
          Stop
        </button>
      )}
      <button
        className="px-3 py-1.5 text-sm font-medium text-white bg-red-600 hover:bg-red-700 rounded-lg transition-colors disabled:opacity-50"
        disabled={loading}
        onClick={() => onAction(vmId, "delete")}
      >
        Delete
      </button>
    </div>
  );
}
