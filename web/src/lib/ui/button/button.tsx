/**
 * Button — lib/ui smoke-test component.
 *
 * The only place className and Tailwind utilities live. Consumers (modules)
 * pass directive props (variant, size) — never className or style.
 *
 * Design-session note: skinning (exact Tailwind classes, token mapping) is
 * owned by the design session (campbooks-b4). This file sets up the CVA
 * scaffold and a minimal default so the build compiles; the design session
 * will restyle it without changing the prop API.
 */
import { type FC, type ButtonHTMLAttributes } from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "~/lib/utils";

// Variant definitions use Campbooks token names from tokens.css.
// The design session owns the exact Tailwind values; these are placeholders.
const buttonVariants = cva(
  // base
  "inline-flex items-center justify-center gap-1.5 rounded-[--cb-r1] text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[--ring] disabled:pointer-events-none disabled:opacity-40",
  {
    variants: {
      variant: {
        default:
          "bg-[--primary] text-[--primary-foreground] hover:opacity-90",
        secondary:
          "bg-[--secondary] text-[--secondary-foreground] border border-[--border] hover:bg-[--muted]",
        ghost:
          "hover:bg-[--accent] hover:text-[--accent-foreground]",
        danger:
          "bg-[--destructive] text-[--destructive-foreground] hover:opacity-90",
      },
      size: {
        sm: "h-[26px] px-2.5 text-xs",
        md: "h-8 px-3",
        lg: "h-[38px] px-4",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "md",
    },
  },
);

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> &
  VariantProps<typeof buttonVariants> & {
    /** Render the button as a child component (Radix Slot pattern). */
    asChild?: boolean;
  };

/**
 * Primary action trigger. Modules receive directive props; className is
 * internal — only lib/ui components may compose raw Tailwind classes.
 */
export const Button: FC<ButtonProps> = ({
  variant,
  size,
  asChild = false,
  className,
  ...props
}) => {
  const Comp = asChild ? Slot : "button";
  return (
    <Comp
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    />
  );
};
