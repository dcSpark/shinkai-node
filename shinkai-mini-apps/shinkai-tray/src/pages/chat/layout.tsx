import { useState } from "react";
import { useForm } from "react-hook-form";
import { Link, Outlet, useMatch } from "react-router-dom";

import { zodResolver } from "@hookform/resolvers/zod";
import { DialogClose } from "@radix-ui/react-dialog";
import { getMessageContent, isJobInbox } from "@shinkai_network/shinkai-message-ts/utils";
import {
  CheckIcon,
  Edit,
  Edit2,
  Edit3,
  EditIcon,
  MessageCircleIcon,
  Workflow,
} from "lucide-react";
import { z } from "zod";

import { useGetInboxes } from "../../api/queries/getInboxes/useGetInboxes";
import { Button } from "../../components/ui/button";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
} from "../../components/ui/form";
import { Input } from "../../components/ui/input";
import { ScrollArea } from "../../components/ui/scroll-area";
import { Separator } from "../../components/ui/separator";
import { cn } from "../../lib/utils";
import { useAuth } from "../../store/auth";

const updateInboxNameSchema = z.object({
  inboxName: z.string(),
});

const InboxNameInput = ({
  closeEditable,
  inboxId,
  inboxName,
}: {
  closeEditable: () => void;
  inboxId: string;
  inboxName: string;
}) => {
  const updateInboxNameForm = useForm<z.infer<typeof updateInboxNameSchema>>({
    resolver: zodResolver(updateInboxNameSchema),
    defaultValues: {
      inboxName,
    },
  });

  const onSubmit = (data: z.infer<typeof updateInboxNameSchema>) => {
    console.log(data);
    closeEditable();
  };

  return (
    <Form {...updateInboxNameForm}>
      <form
        className="relative flex h-[46px] items-center"
        onSubmit={updateInboxNameForm.handleSubmit(onSubmit)}
      >
        <div className="space-y-1 pr-10">
          <FormField
            render={({ field }) => (
              <FormItem className="space-y-0 text-xs">
                <FormLabel className="sr-only">Update inbox name</FormLabel>
                <FormControl>
                  <Input className="text-xs" placeholder="Eg: Work Inbox " {...field} />
                </FormControl>
              </FormItem>
            )}
            control={updateInboxNameForm.control}
            name="inboxName"
          />
        </div>

        <Button
          className="absolute right-0 top-1/2 -translate-y-1/2 transform"
          size="icon"
          type="submit"
          variant="default"
        >
          <CheckIcon className="h-4 w-4" />
        </Button>
      </form>
    </Form>
  );
};

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

  const [isEditable, setIsEditable] = useState(false);

  return isEditable ? (
    <InboxNameInput
      closeEditable={() => setIsEditable(false)}
      inboxId={inboxId}
      inboxName={inboxName}
    />
  ) : (
    <Link
      className={cn(
        "group flex h-[46px] w-full items-center gap-2 rounded-lg px-2 py-2 text-muted-foreground hover:bg-slate-800",
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
      <span className="line-clamp-1 flex-1 text-left text-xs">{inboxName}</span>
      <Button
        className={cn("hidden justify-self-end", match && "flex")}
        onClick={() => setIsEditable(true)}
        size="icon"
        variant="ghost"
      >
        <Edit className="h-4 w-4" />
      </Button>
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
              <div className="space-y-1">
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
