/**
 * SkeletonRow — loading placeholder that matches the InboxRow layout.
 */
import { type FC } from "react";
import { cn } from "~/lib/utils";

const Bone: FC<{ className?: string }> = ({ className }) => (
  <div
    className={cn(
      "bg-surface-2 rounded-cb-1 animate-pulse",
      className,
    )}
    aria-hidden="true"
  />
);

export const SkeletonRow: FC = () => (
  <div className="flex items-start gap-3 px-4 py-3 border-b border-line">
    {/* Avatar */}
    <Bone className="w-8 h-8 rounded-full shrink-0 mt-0.5" />
    {/* Content */}
    <div className="flex-1 min-w-0 flex flex-col gap-1.5">
      <div className="flex items-center gap-2">
        <Bone className="h-3 w-24" />
        <Bone className="h-2.5 w-8 ml-auto" />
      </div>
      <Bone className="h-2.5 w-48" />
      <Bone className="h-2.5 w-32" />
    </div>
  </div>
);
