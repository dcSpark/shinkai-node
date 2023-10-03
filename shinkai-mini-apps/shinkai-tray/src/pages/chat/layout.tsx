import { Link, Outlet, useMatch, useResolvedPath } from "react-router-dom";

import { MessageCircleIcon } from "lucide-react";

import { useGetInboxes } from "../../api/queries/getInboxes/useGetInboxes";
import { ScrollArea } from "../../components/ui/scroll-area";
import { Separator } from "../../components/ui/separator";
import { cn } from "../../lib/utils";
import { useAuth } from "../../store/auth-context";

const MessageButton = ({ to, inboxId }: { to: string; inboxId: string }) => {
  const resolved = useResolvedPath(to);
  const match = useMatch({ path: resolved.pathname, end: true });

  return (
    <Link
      className={cn(
        "flex w-full items-center gap-2 rounded-lg px-2 py-2 text-muted-foreground hover:bg-slate-800",
        match && "bg-slate-800 text-foreground"
      )}
      key={inboxId}
      to={to}
    >
      <MessageCircleIcon className="h-4 w-4 shrink-0" />
      <span className="line-clamp-1 text-left text-xs">
        {decodeURIComponent(inboxId)}
      </span>
    </Link>
  );
};

const ChatLayout = () => {
  const { setupData } = useAuth();

  const { inboxIds } = useGetInboxes({
    sender: setupData?.shinkai_identity ?? "",
    senderSubidentity: `${setupData?.profile}/device/${setupData?.registration_name}`,
    shinkaiIdentity: setupData?.shinkai_identity ?? "",
    my_device_encryption_sk: setupData?.my_device_encryption_sk ?? "",
    my_device_identity_sk: setupData?.my_device_identity_sk ?? "",
    node_encryption_pk: setupData?.node_encryption_pk ?? "",
    // Assuming receiver and target_shinkai_name_profile are the same as sender
    receiver: setupData?.shinkai_identity ?? "",
    targetShinkaiNameProfile: `${setupData?.shinkai_identity}/${setupData?.profile}`,
  });

  return (
    <div className="flex h-full">
      {inboxIds.length > 0 ? (
        <>
          <div className="flex max-w-[280px] flex-[280px] shrink-0 flex-col px-2 py-4">
            <h2 className="mb-4 px-2">Recent Conversations</h2>
            <ScrollArea>
              <div className="space-y-2">
                {inboxIds.map((inboxId) => (
                  <MessageButton
                    inboxId={inboxId}
                    key={inboxId}
                    to={`/inboxes/${inboxId}`}
                  />
                ))}
              </div>
            </ScrollArea>
          </div>
          <Separator orientation="vertical" />
        </>
      ) : null}
      <Outlet />
    </div>
  );
};

export default ChatLayout;
