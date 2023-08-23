import { IonAvatar } from "@ionic/react";
import { cn } from "../../theme/lib/utils";

export default function Avatar({
  url,
  className,
}: {
  url?: string;
  className?: string;
}) {
  return (
    <IonAvatar className={cn("bg-white w-10 h-10 border-0", className)}>
      <img
        className="w-full h-full"
        src={url ?? "https://ionicframework.com/docs/img/demos/avatar.svg"}
        alt=""
      />
    </IonAvatar>
  );
}
