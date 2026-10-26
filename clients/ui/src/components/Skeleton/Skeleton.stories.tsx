import type { Meta, StoryObj } from "@storybook/react-vite";

import { Skeleton, SkeletonList } from "./Skeleton";

const meta = {
  title: "Components/Skeleton",
  component: Skeleton,
} satisfies Meta<typeof Skeleton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Line: Story = { args: { width: "60%" } };
export const AvatarShape: Story = { args: { shape: "avatar" } };

/** Queue-shaped loading rows, announced once. Shimmer stops under reduced motion. */
export const List: Story = { render: () => <SkeletonList rows={3} /> };
