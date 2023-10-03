import { Fragment, useCallback, useEffect, useRef } from "react";
import { useForm } from "react-hook-form";
import { useParams } from "react-router-dom";

import { zodResolver } from "@hookform/resolvers/zod";
import { DotsVerticalIcon, PaperPlaneIcon } from "@radix-ui/react-icons";
import {
  calculateMessageHash,
  extractJobIdFromInbox,
  extractReceiverShinkaiName,
  isJobInbox,
} from "@shinkai_network/shinkai-message-ts/utils";
import { Placeholder } from "@tiptap/extension-placeholder";
import { EditorContent, useEditor } from "@tiptap/react";
import { StarterKit } from "@tiptap/starter-kit";
import MarkdownPreview from "@uiw/react-markdown-preview";
import { Loader } from "lucide-react";
import { Markdown } from "tiptap-markdown";
import { z } from "zod";

import { useSendMessageToJob } from "../../api/mutations/sendMessageToJob/useSendMessageToJob";
// import { useGetLastUnreadMessages } from "../../api/queries/getLastUnreadMessages/useGetLastUnreadMessages";
import { useSendMessageToInbox } from "../../api/mutations/sendTextMessage/useSendMessageToInbox";
import { useGetChatConversationWithPagination } from "../../api/queries/getChatConversation/useGetChatConversationWithPagination";
import { Avatar, AvatarFallback, AvatarImage } from "../../components/ui/avatar";
import { Button } from "../../components/ui/button";
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
} from "../../components/ui/form";
import { ScrollArea } from "../../components/ui/scroll-area";
import { Skeleton } from "../../components/ui/skeleton";
import { useAuth } from "../../store/auth-context";

const chatSchema = z.object({
  message: z.string(),
});

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const getMessageFromJob = (message: any) => {
  if ("unencrypted" in message.body) {
    return JSON.parse(
      message.body.unencrypted.message_data.unencrypted.message_raw_content
    ).content;
  }
  return message.body.unencrypted.message_data.encrypted.content;
};
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const getMessageFromChat = (message: any) => {
  return message.body.unencrypted.message_data.unencrypted.message_raw_content;
};

const ChatConversation = () => {
  const { inboxId: encodedInboxId = "" } = useParams();
  const { setupData } = useAuth();
  const chatContainerRef = useRef<HTMLDivElement | null>(null);
  const previousChatHeightRef = useRef<number>(0);
  const fromPreviousMessagesRef = useRef<boolean>(false);

  const inboxId = decodeURIComponent(encodedInboxId);
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
    isLoading: isChatConversationLoading,
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

  const { mutateAsync: sendMessageToInbox, isLoading: isSendingMessageToInbox } =
    useSendMessageToInbox();
  const { mutateAsync: sendMessageToJob, isLoading: isSendingMessageToJob } =
    useSendMessageToJob();

  const onSubmit = async (data: z.infer<typeof chatSchema>) => {
    if (!setupData) return;
    fromPreviousMessagesRef.current = false;
    if (isJobInbox(inboxId)) {
      const sender = `${setupData.shinkai_identity}/${setupData.profile}`;
      const jobId = extractJobIdFromInbox(inboxId);
      await sendMessageToJob({
        jobId,
        message: data.message,
        sender,
        files_inbox: "",
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
        my_device_encryption_sk: setupData.profile_encryption_sk,
        my_device_identity_sk: setupData.profile_identity_sk,
        node_encryption_pk: setupData.node_encryption_pk,
      });
    }
    chatForm.reset();
  };

  const isLoading = isSendingMessageToJob || isSendingMessageToInbox;

  const fetchPreviousMessages = useCallback(async () => {
    const firstMessage = data?.pages?.[0]?.[0];
    fromPreviousMessagesRef.current = true;
    if (!firstMessage) return;
    const timeKey = firstMessage?.external_metadata?.scheduled_time;
    const hashKey = calculateMessageHash(firstMessage);
    const firstMessageKey = `${timeKey}:::${hashKey}`;
    await fetchPreviousPage({ pageParam: { lastKey: firstMessageKey } });
  }, [data?.pages, fetchPreviousPage]);

  const handleScroll = useCallback(async () => {
    const chatContainerElement = chatContainerRef.current;
    if (!chatContainerElement) return;
    const currentHeight = chatContainerElement.scrollHeight;
    const previousHeight = previousChatHeightRef.current;

    if (chatContainerElement.scrollTop < 100 && hasPreviousPage) {
      await fetchPreviousMessages();
      previousChatHeightRef.current = currentHeight;
      chatContainerElement.scrollTop = currentHeight - previousHeight;
    }
  }, [fetchPreviousMessages, hasPreviousPage]);

  useEffect(() => {
    const chatContainerElement = chatContainerRef.current;
    if (!chatContainerElement) return;
    chatContainerElement.addEventListener("scroll", handleScroll);
    return () => {
      chatContainerElement.removeEventListener("scroll", handleScroll);
    };
  }, [handleScroll]);

  const scrollToBottom = () => {
    if (!chatContainerRef.current) return;
    chatContainerRef.current.scrollTop = chatContainerRef.current.scrollHeight;
  };
  useEffect(() => {
    if (!fromPreviousMessagesRef.current) {
      scrollToBottom();
    }
  }, [data?.pages]);

  return (
    <div className="flex flex-1 flex-col pt-2">
      <div className="mb-3 flex shrink-0 items-center gap-2 px-4">
        <Avatar className="h-7 w-7">
          <AvatarImage
            alt="Shinkai AI"
            className="h-7 w-7"
            src={`https://ui-avatars.com/api/?name=${inboxId}&background=0b1115&color=c7c7c7`}
          />
          <AvatarFallback className="h-7 w-7" />
        </Avatar>
        <span className="flex-1 text-left text-sm">{inboxId}</span>
        <Button size="icon" variant="ghost">
          <span className="sr-only">Settings</span>
          <DotsVerticalIcon className="h-4 w-4" />
        </Button>
      </div>
      <ScrollArea className="h-full px-4" ref={chatContainerRef}>
        {isChatConversationSuccess && (
          <div className="py-2 text-center text-xs">
            {isFetchingPreviousPage || hasPreviousPage ? (
              <Loader className="flex animate-spin justify-center text-white" />
            ) : (
              "All messages has been loaded 🎈 "
            )}
          </div>
        )}
        <div className="space-y-5">
          {isChatConversationLoading &&
            [1, 2, 3, 4].map((index) => (
              <Skeleton className="h-10 w-full rounded-lg" key={index} />
            ))}
          {isChatConversationSuccess &&
            data?.pages.map((group, index) => (
              <Fragment key={index}>
                {group.map((message) => {
                  // const localIdentity = `${setupData?.profile}/device/${setupData?.registration_name}`;
                  // let isLocalMessage = false;
                  // if (message.body && "unencrypted" in message.body) {
                  //   isLocalMessage =
                  //     message.body.unencrypted.internal_metadata.sender_subidentity ===
                  //     localIdentity;
                  // }
                  return (
                    <div
                      className="rounded-lg bg-[rgba(217,217,217,0.04)] px-4 py-6"
                      key={message.external_metadata?.scheduled_time}
                    >
                      {/* <p
                        className={cn(
                          "text-sm",
                          isLocalMessage ? "text-muted-foreground" : "text-foreground"
                        )}
                      >
                        {isJobInbox(inboxId)
                          ? getMessageFromJob(message)
                          : getMessageFromChat(message)}
                      </p> */}
                      <MarkdownPreview
                        source={
                          isJobInbox(inboxId)
                            ? getMessageFromJob(message)
                            : getMessageFromChat(message)
                        }
                        className="bg-transparent text-sm text-foreground"
                      />
                      {/* <p className="text-xs">
                        <span className="text-muted-foreground">Sent at </span>
                        <span className=" text-gray-600">
                        {new Date(
                          message?.external_metadata?.scheduled_time ?? ""
                          ).toLocaleString()}
                          </span>
                        </p> */}
                    </div>
                  );
                })}
              </Fragment>
            ))}
        </div>
      </ScrollArea>

      <div className="flex flex-col justify-start">
        <div className="flex items-start gap-2 bg-app-gradient p-2 pt-3">
          {/* <EditorContent editor={editor} /> */}

          <Form {...chatForm}>
            <FormField
              render={({ field }) => (
                <FormItem className="flex-1 space-y-0">
                  <FormLabel className="sr-only">Enter message</FormLabel>
                  <FormControl>
                    <MessageEditor
                      disabled={isLoading}
                      onChange={field.onChange}
                      onSubmit={chatForm.handleSubmit(onSubmit)}
                      value={field.value}
                    />
                  </FormControl>
                  <FormDescription className="pt-1 text-xs">
                    Press <kbd>⌘</kbd> <kbd>↵</kbd> to send message
                  </FormDescription>
                </FormItem>
              )}
              control={chatForm.control}
              name="message"
            />

            <Button
              disabled={isLoading}
              isLoading={isLoading}
              onClick={chatForm.handleSubmit(onSubmit)}
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

const MessageEditor = ({
  value,
  onChange,
  onSubmit,
  setInitialValue,
  disabled,
}: {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  setInitialValue?: string;
  disabled?: boolean;
}) => {
  const editor = useEditor({
    editable: !disabled,
    editorProps: {
      attributes: {
        class: "prose prose-invert prose-sm mx-auto focus:outline-none",
      },
      handleDOMEvents: {
        keydown: (_, event) => {
          if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
            event.preventDefault();
            onSubmit?.();
          }
        },
      },
    },
    extensions: [
      StarterKit,
      Placeholder.configure({
        placeholder: "Enter message",
      }),
      Markdown,
    ],
    content: value,

    onUpdate({ editor }) {
      // onChange(editor.getHTML());
      onChange(editor.storage.markdown.getMarkdown());
    },
  });

  useEffect(() => {
    setInitialValue === undefined
      ? editor?.commands.setContent("")
      : editor?.commands.setContent(value);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [setInitialValue]);

  useEffect(() => {
    if (value === "") editor?.commands.setContent("");
  }, [value, editor]);

  return <EditorContent className="prose-" editor={editor} />;
};
