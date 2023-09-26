import { Separator } from "../components/ui/separator";
import { useAuth } from "../store/auth-context";
import { MessageCircleIcon } from "lucide-react";
import { useGetInboxes } from "../api/queries/getInboxes/useGetInboxes";
import { Outlet, useNavigate } from "react-router-dom";

const HomePage = () => {
  const { setupData } = useAuth();
  const navigate = useNavigate();
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

  const goToInboxDetails = (inboxId: string) => {
    navigate(`/inboxes/${encodeURIComponent(inboxId)}`);
  };

  return (
    <div className="flex h-full">
      <div className="flex-[380px] max-w-[380px] py-4 px-2 flex flex-col justify-between">
        <div>
          <h2 className="mb-4 px-2">Recent Conversations</h2>
          <div className="space-y-2">
            {inboxIds.map((inboxId) => (
              <button
                key={inboxId}
                onClick={() => goToInboxDetails(inboxId)}
                className="flex w-full items-center gap-2 px-2 py-2 hover:bg-slate-800 rounded-lg"
              >
                <MessageCircleIcon className="shrink-0 w-4 h-4" />
                <span className="text-left text-xs line-clamp-1">{inboxId}</span>
              </button>
            ))}
          </div>
        </div>
      </div>
      <Separator orientation="vertical" />
      <Outlet />
    </div>
  );
};

export default HomePage;
