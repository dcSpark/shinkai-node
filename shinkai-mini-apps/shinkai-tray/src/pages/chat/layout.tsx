import { Link, Outlet, useMatch, useResolvedPath } from "react-router-dom";
import { Separator } from "../../components/ui/separator";
import { MessageCircleIcon } from "lucide-react";
import { useAuth } from "../../store/auth-context";
import { useGetInboxes } from "../../api/queries/getInboxes/useGetInboxes";
import { cn } from "../../lib/utils";

const MessageButton = ({ to, inboxId }: { to: string; inboxId: string }) => {
  const resolved = useResolvedPath(to);
  const match = useMatch({ path: resolved.pathname, end: true });
  console.log(resolved.pathname, "pathname", match);

  return (
    <Link
      key={inboxId}
      to={to}
      className={cn(
        "text-muted-foreground flex w-full items-center gap-2 px-2 py-2 hover:bg-slate-800 rounded-lg",
        match && "bg-slate-800 text-foreground"
      )}
    >
      <MessageCircleIcon className="shrink-0 w-4 h-4" />
      <span className="text-left text-xs line-clamp-1">{inboxId}</span>
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
          <div className="flex-[380px] max-w-[380px] py-4 px-2 flex flex-col justify-between">
            <h2 className="mb-4 px-2">Recent Conversations</h2>
            <div className="space-y-2 flex-1 overflow-auto">
              {inboxIds.map((inboxId) => (
                <MessageButton
                  key={inboxId}
                  inboxId={inboxId}
                  to={`/inboxes/${inboxId}`}
                />
              ))}
            </div>
          </div>
          <Separator orientation="vertical" />{" "}
        </>
      ) : null}
      <Outlet />
    </div>
  );
};

export default ChatLayout;
