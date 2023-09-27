import { useParams } from "react-router-dom";
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
// import { useGetLastUnreadMessages } from "../../api/queries/getLastUnreadMessages/useGetLastUnreadMessages";
import { cn } from "../../lib/utils";
import { useSendMessageToInbox } from "../../api/mutations/sendTextMessage/useSendMessageToInbox";
import {
  calculateMessageHash,
  extractJobIdFromInbox,
  extractReceiverShinkaiName,
  isJobInbox,
} from "@shinkai_network/shinkai-message-ts/utils";
import { useSendMessageToJob } from "../../api/mutations/sendMessageToJob/useSendMessageToJob";
import { Fragment, useCallback, useEffect, useLayoutEffect, useRef } from "react";
import { ScrollArea } from "../../components/ui/scroll-area";
import { Loader } from "lucide-react";
import { useGetChatConversationWithPagination } from "../../api/queries/getChatConversation/useGetChatConversationWithPagination";

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
  const { inboxId = "" } = useParams();
  const { setupData } = useAuth();
  const chatContainerRef = useRef<HTMLDivElement | null>(null);
  const prevChatHeightRef = useRef<number>(0);

  const chatForm = useForm<z.infer<typeof chatSchema>>({
    resolver: zodResolver(chatSchema),
    defaultValues: {
      message: "",
    },
  });

  const {
    data,
    fetchPreviousPage,
    hasPreviousPage,
    isFetching,
    isFetchingPreviousPage,
    isSuccess: isChatConversationSuccess,
  } = useGetChatConversationWithPagination({
    inboxId: inboxId as string,
    shinkaiIdentity: setupData?.shinkai_identity ?? "",
    profile: setupData?.profile ?? "",
    my_device_encryption_sk: setupData?.my_device_encryption_sk ?? "",
    my_device_identity_sk: setupData?.my_device_identity_sk ?? "",
    node_encryption_pk: setupData?.node_encryption_pk ?? "",
    profile_encryption_sk: setupData?.profile_encryption_sk ?? "",
    profile_identity_sk: setupData?.profile_identity_sk ?? "",
  });

  const {
    mutateAsync: sendMessageToInbox,
    isLoading: isSendingMessageToInbox,
    isSuccess: isSendingMessageToInboxSuccess,
  } = useSendMessageToInbox();
  const {
    mutateAsync: sendMessageToJob,
    isLoading: isSendingMessageToJob,
    isSuccess: isSendingMessageToJobSuccess,
  } = useSendMessageToJob();

  const onSubmit = async (data: z.infer<typeof chatSchema>) => {
    if (!setupData) return;
    if (isJobInbox(inboxId)) {
      const sender = `${setupData.shinkai_identity}/${setupData.profile}`;
      const jobId = extractJobIdFromInbox(inboxId);
      await sendMessageToJob({
        jobId,
        message: data.message,
        sender,
        shinkaiIdentity: setupData.shinkai_identity,
        my_device_encryption_sk: setupData.my_device_encryption_sk,
        my_device_identity_sk: setupData.my_device_identity_sk,
        node_encryption_pk: setupData.node_encryption_pk,
        profile_encryption_sk: setupData.profile_encryption_sk,
        profile_identity_sk: setupData.profile_identity_sk,
      });
    } else {
      const sender = `${setupData.shinkai_identity}/${setupData.profile}/device/${setupData.registration_name}`;
      const receiver = extractReceiverShinkaiName(inboxId, sender);
      await sendMessageToInbox({
        sender,
        receiver,
        message: data.message,
        inboxId: inboxId as string,
        my_device_encryption_sk: setupData.my_device_encryption_sk,
        my_device_identity_sk: setupData.my_device_identity_sk,
        node_encryption_pk: setupData.node_encryption_pk,
      });
    }
    chatForm.reset();
  };

  const isLoading = isSendingMessageToJob || isSendingMessageToInbox;

  const fetchPreviousMessages = useCallback(async () => {
    if (!hasPreviousPage) return;
    const firstMessage = data?.pages?.[0]?.[0];
    if (!firstMessage) return;
    const timeKey = firstMessage?.external_metadata?.scheduled_time;
    const hashKey = calculateMessageHash(firstMessage);
    const firstMessageKey = `${timeKey}:::${hashKey}`;
    await fetchPreviousPage({ pageParam: { lastKey: firstMessageKey } });
  }, [data?.pages, fetchPreviousPage, hasPreviousPage]);

  const handleScroll = useCallback(async () => {
    const chatContainerElement = chatContainerRef.current;
    if (!chatContainerElement) return;
    const currentHeight = chatContainerElement.scrollHeight;
    const prevHeight = prevChatHeightRef.current;

    if (chatContainerElement.scrollTop < 100 && hasPreviousPage) {
      await fetchPreviousMessages();
      prevChatHeightRef.current = currentHeight;
      chatContainerElement.scrollTop = currentHeight - prevHeight;
    }
  }, [fetchPreviousMessages, hasPreviousPage]);

  useEffect(() => {
    const chatContainerElement = chatContainerRef.current;
    if (!chatContainerElement) return;
    chatContainerElement.addEventListener("scroll", handleScroll);
    return () => {
      chatContainerElement.removeEventListener("scroll", handleScroll);
    };
  }, [
    hasPreviousPage,
    isFetching,
    isFetchingPreviousPage,
    isChatConversationSuccess,
    handleScroll,
  ]);

  useLayoutEffect(() => {
    if (chatContainerRef.current) {
      chatContainerRef.current.scrollTop = chatContainerRef.current.scrollHeight;
    }
  }, [
    isSendingMessageToJobSuccess,
    isSendingMessageToInboxSuccess,
    isChatConversationSuccess,
  ]);

  return (
    <div className="w-full flex flex-col justify-between pt-2">
      <div className="px-4 mb-3 shrink-0 flex items-center gap-2">
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
      <ScrollArea className="h-full px-4 " ref={chatContainerRef}>
        <Button
          variant="ghost"
          className="inline mx-auto"
          disabled={!hasPreviousPage || isFetching}
          onClick={fetchPreviousMessages}
        >
          {isFetchingPreviousPage ? (
            <Loader className="flex w-full justify-center text-white" />
          ) : hasPreviousPage ? (
            "Load previous"
          ) : (
            "All messages has been loaded."
          )}
        </Button>
        <div className="chat-messages space-y-5 ">
          {data?.pages.map((group, i) => (
            <Fragment key={i}>
              {group.map((message) => {
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
                    className="message-chat flex items-center gap-2 rounded-lg p-2 py-6 bg-[rgba(217,217,217,0.04)]"
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
            </Fragment>
          ))}
        </div>
      </ScrollArea>

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
              onClick={chatForm.handleSubmit(onSubmit)}
              isLoading={isLoading}
              disabled={isLoading}
              size="icon"
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
