import { useParams } from "react-router-dom";
import { useGetChatConversation } from "../../api/queries/getChatConversation/useGetChatConversation";
import { useAuth } from "../../store/auth-context";
import { Avatar, AvatarFallback, AvatarImage } from "../../components/ui/avatar";
import { Textarea } from "../../components/ui/textarea";
import { DotsVerticalIcon, PaperPlaneIcon } from "@radix-ui/react-icons";
import { Button } from "../../components/ui/button";
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
} from "../../components/ui/form";
import { z } from "zod";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { useGetLastUnreadMessages } from "../../api/queries/getLastUnreadMessages/useGetLastUnreadMessages";
import { cn } from "../../lib/utils";

const chatSchema = z.object({
  message: z.string(),
});

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const parseMessage = (message: any) => {
  if (!message.body) {
    return message.body?.encrypted.content;
  }
  if ("unencrypted" in message.body) {
    return JSON.parse(
      message.body.unencrypted.message_data.unencrypted.message_raw_content
    ).content;
  }
  return message.body.unencrypted.message_data.encrypted.content;
};

const ChatConversation = () => {
  const { inboxId } = useParams();
  const { setupData } = useAuth();
  console.log(inboxId, "inboxId");
  const chatForm = useForm<z.infer<typeof chatSchema>>({
    resolver: zodResolver(chatSchema),
    defaultValues: {
      message: "",
    },
  });

  const onSubmit = (data: z.infer<typeof chatSchema>) => {
    console.log(data);
  };

  const { data: messages } = useGetChatConversation({
    inboxId: inboxId as string,
    count: 10,
    lastKey: undefined,
    shinkaiIdentity: setupData?.shinkai_identity ?? "",
    profile: setupData?.profile ?? "",
    my_device_encryption_sk: setupData?.my_device_encryption_sk ?? "",
    my_device_identity_sk: setupData?.my_device_identity_sk ?? "",
    node_encryption_pk: setupData?.node_encryption_pk ?? "",
    profile_encryption_sk: setupData?.profile_encryption_sk ?? "",
    profile_identity_sk: setupData?.profile_identity_sk ?? "",
  });

  console.log(messages, "messages-regular");
  const { data: messagesLast } = useGetLastUnreadMessages({
    inboxId: inboxId as string,
    count: 10,
    lastKey: undefined,
    shinkaiIdentity: setupData?.shinkai_identity ?? "",
    profile: setupData?.profile ?? "",
    my_device_encryption_sk: setupData?.my_device_encryption_sk ?? "",
    my_device_identity_sk: setupData?.my_device_identity_sk ?? "",
    node_encryption_pk: setupData?.node_encryption_pk ?? "",
    profile_encryption_sk: setupData?.profile_encryption_sk ?? "",
    profile_identity_sk: setupData?.profile_identity_sk ?? "",
  });

  console.log(messagesLast, "messages-unread");

  return (
    <div className="w-full flex flex-col justify-between">
      <div className="px-4 py-3">
        <h1 className="sr-only">Chat Conversation</h1>
        <div className="flex items-center gap-2">
          <Avatar className="w-7 h-7">
            <AvatarImage
              src={`https://ui-avatars.com/api/?name=${inboxId}&background=0b1115&color=c7c7c7`}
              alt="Shinkai AI"
              className="w-7 h-7"
            />
            <AvatarFallback className="w-7 h-7" />
          </Avatar>
          <span className="text-sm flex-1 text-left">{inboxId}</span>
          <Button variant="ghost" size="icon">
            <span className="sr-only">Settings</span>
            <DotsVerticalIcon className="w-4 h-4" />
          </Button>
        </div>
        <div className="mt-4 ">
          {messages?.map((message) => {
            const localIdentity = `${setupData?.profile}/device/${setupData?.registration_name}`;

            let isLocalMessage = false;

            if (message.body && "unencrypted" in message.body) {
              isLocalMessage =
                message.body.unencrypted.internal_metadata.sender_subidentity ===
                localIdentity;
            }

            return (
              <div
                key={message.external_metadata?.scheduled_time}
                className="flex items-center gap-2 rounded-lg p-2 bg-[rgba(217,217,217,0.04)]"
              >
                <p
                  className={cn(
                    "text-sm",
                    isLocalMessage ? "text-muted-foreground" : "text-foreground"
                  )}
                >
                  {parseMessage(message)}
                </p>
                <span className="text-xs text-gray-600">
                  {new Date(
                    message?.external_metadata?.scheduled_time ?? ""
                  ).toLocaleDateString()}
                </span>
              </div>
            );
          })}
        </div>
      </div>
      <div className="flex flex-col justify-start">
        <div className="bg-app-gradient p-2 pt-3 flex items-start gap-2">
          <Form {...chatForm}>
            <FormField
              control={chatForm.control}
              name="message"
              render={({ field }) => (
                <FormItem className="space-y-0 flex-1">
                  <FormLabel className="sr-only">Enter message</FormLabel>
                  <FormControl>
                    <Textarea
                      placeholder="Ask Shinkai AI"
                      onKeyDown={(e) => {
                        console.log("aaa");
                        if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                          chatForm.handleSubmit(onSubmit)();
                        }
                      }}
                      {...field}
                    />
                  </FormControl>
                  <FormDescription className="text-xs pt-1">
                    Press <kbd>⌘</kbd> <kbd>↵</kbd> to send message
                  </FormDescription>
                </FormItem>
              )}
            />

            <Button
              className="w-10 h-10 p-0 shrink-0"
              onClick={chatForm.handleSubmit(onSubmit)}
            >
              <PaperPlaneIcon />
              <span className="sr-only">Send Message</span>
            </Button>
          </Form>
        </div>
      </div>
    </div>
  );
};

export default ChatConversation;
