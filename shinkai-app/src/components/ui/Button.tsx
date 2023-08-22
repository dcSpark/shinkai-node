import React from "react";
import { IonButton } from "@ionic/react";
import { cn } from "../../theme/lib/utils";

export default function Button({
  onClick,
  disabled,
  children,
  className,
}: {
  onClick: () => void;
  disabled?: boolean;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <IonButton
      className={cn(
        "w-full",
        "[--border-radius:16px] [--box-shadow:none]",
        className,
      )}
      onClick={onClick}
      disabled={disabled}
    >
      {children}
    </IonButton>
  );
}
