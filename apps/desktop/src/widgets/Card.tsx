import { Card as HeroCard } from "@heroui/react";
import { cn } from "@heroui/react";
import type { ReactNode } from "react";

/**
 * Thin pass-through over HeroUI `Card`. We keep this wrapper almost style-free
 * so the app inherits HeroUI's native card look (rounded corners, surface
 * background, soft shadow, default 16px padding + gap).
 *
 * Two card shapes exist in the app:
 *   1. Content cards — a simple padded box (use `Card` + `Card.Title`/children).
 *      These rely on HeroUI's native padding (callers may pass p-4/p-5).
 *   2. List/table cards — a header with a full-bleed bottom divider followed by
 *      full-width rows. HeroUI's Card doesn't model this, so those callers pass
 *      `p-0 gap-0` to drop the native inset, and `Card.Header` (used ONLY by
 *      these) supplies the header padding + divider. Row padding comes from the
 *      `.card-row` class.
 */
function CardRoot({
  className,
  children,
  ...props
}: {
  className?: string;
  children: ReactNode;
} & React.HTMLAttributes<HTMLDivElement>) {
  return (
    <HeroCard className={className} {...props}>
      {children}
    </HeroCard>
  );
}

function CardHeader({
  className,
  children,
  ...props
}: {
  className?: string;
  children: ReactNode;
} & React.HTMLAttributes<HTMLDivElement>) {
  return (
    <HeroCard.Header
      className={cn(
        "flex flex-row items-center justify-between gap-3 border-b border-[var(--line)] px-4 py-3.5",
        className,
      )}
      {...props}
    >
      {children}
    </HeroCard.Header>
  );
}

function CardTitle({
  className,
  children,
  ...props
}: {
  className?: string;
  children: ReactNode;
} & React.HTMLAttributes<HTMLHeadingElement>) {
  return (
    <HeroCard.Title className={className} {...props}>
      {children}
    </HeroCard.Title>
  );
}

function CardSubtitle({
  className,
  children,
  ...props
}: {
  className?: string;
  children: ReactNode;
} & React.HTMLAttributes<HTMLParagraphElement>) {
  return (
    <HeroCard.Description className={className} {...props}>
      {children}
    </HeroCard.Description>
  );
}

function CardBody({
  className,
  children,
  ...props
}: {
  className?: string;
  children: ReactNode;
} & React.HTMLAttributes<HTMLDivElement>) {
  return (
    <HeroCard.Content className={cn("p-4", className)} {...props}>
      {children}
    </HeroCard.Content>
  );
}

export const Card = Object.assign(CardRoot, {
  Header: CardHeader,
  Title: CardTitle,
  Subtitle: CardSubtitle,
  Body: CardBody,
});
