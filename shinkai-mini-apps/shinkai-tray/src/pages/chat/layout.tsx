import { useState } from "react";
import { Link, Outlet, useMatch } from "react-router-dom";

import { DialogClose } from "@radix-ui/react-dialog";
import { getMessageContent, isJobInbox } from "@shinkai_network/shinkai-message-ts/utils";
import { Edit2, EditIcon, MessageCircleIcon, Workflow } from "lucide-react";

import { useGetInboxes } from "../../api/queries/getInboxes/useGetInboxes";
import { Button } from "../../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "../../components/ui/dialog";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { ScrollArea } from "../../components/ui/scroll-area";
import { Separator } from "../../components/ui/separator";
import { cn } from "../../lib/utils";
import { useAuth } from "../../store/auth";

const MessageButton = ({
  to,
  inboxId,
  inboxName,
}: {
  to: string;
  inboxId: string;
  inboxName: string;
}) => {
  const match = useMatch(to);

  return (
    <Link
      className={cn(
        "flex w-full items-center gap-2 rounded-lg px-2 py-2 text-muted-foreground hover:bg-slate-800",
        match && "bg-slate-800 text-foreground"
      )}
      key={inboxId}
      to={to}
    >
      {isJobInbox(decodeURIComponent(inboxId)) ? (
        <Workflow className="mr-2 h-4 w-4 shrink-0" />
      ) : (
        <MessageCircleIcon className="mr-2 h-4 w-4 shrink-0" />
      )}
      <span className="line-clamp-1 text-left text-xs">{inboxName}</span>
      <Dialog>
        <DialogTrigger asChild>
          <Button size="icon" variant="secondary">
            <Edit2 className="h-4 w-4" />
            <span className="sr-only">Edit Inbox Name</span>
          </Button>
        </DialogTrigger>
        <DialogContent className="sm:max-w-[425px]">
          <DialogHeader>
            <DialogTitle>Edit Inbox Name</DialogTitle>
            <DialogDescription>
              Change the name of this inbox to make it easier to identify.
            </DialogDescription>
          </DialogHeader>
          <div className="py-4">
            <Label className="sr-only" htmlFor="name">
              Name
            </Label>
            <Input className="col-span-3" defaultValue="Pedro Duarte" id="name" />
          </div>
          <DialogFooter>
            <DialogClose asChild>
              <Button type="button" variant="secondary">
                Close
              </Button>
            </DialogClose>
            <Button type="submit">Save changes</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Link>
  );
};

const ChatLayout = () => {
  const auth = useAuth((state) => state.auth);

  const { inboxes } = useGetInboxes({
    sender: auth?.shinkai_identity ?? "",
    senderSubidentity: `${auth?.profile}/device/${auth?.registration_name}`,
    // Assuming receiver and target_shinkai_name_profile are the same as sender
    receiver: auth?.shinkai_identity ?? "",
    targetShinkaiNameProfile: `${auth?.shinkai_identity}/${auth?.profile}`,
    my_device_encryption_sk: auth?.my_device_encryption_sk ?? "",
    my_device_identity_sk: auth?.my_device_identity_sk ?? "",
    node_encryption_pk: auth?.node_encryption_pk ?? "",
    profile_encryption_sk: auth?.profile_encryption_sk ?? "",
    profile_identity_sk: auth?.profile_identity_sk ?? "",
  });

  return (
    <div className="flex h-full">
      {inboxes.length > 0 ? (
        <>
          <div className="flex max-w-[280px] flex-[280px] shrink-0 flex-col px-2 py-4">
            <h2 className="mb-4 px-2">Recent Conversations</h2>
            <ScrollArea>
              <div className="space-y-2">
                {inboxes.map((inbox) => (
                  <MessageButton
                    inboxName={
                      inbox.custom_name === inbox.inbox_id
                        ? getMessageContent(inbox.last_message)?.slice(0, 40)
                        : inbox.custom_name
                    }
                    inboxId={inbox.inbox_id}
                    key={inbox.inbox_id}
                    to={`/inboxes/${inbox.inbox_id}`}
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
