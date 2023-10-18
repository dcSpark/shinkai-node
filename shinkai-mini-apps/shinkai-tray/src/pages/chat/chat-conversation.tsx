import { Fragment, useCallback, useEffect, useRef } from "react";
import { useForm } from "react-hook-form";
import { useParams } from "react-router-dom";

import { zodResolver } from "@hookform/resolvers/zod";
import { PaperPlaneIcon } from "@radix-ui/react-icons";
import {
  calculateMessageHash,
  extractJobIdFromInbox,
  extractReceiverShinkaiName,
  getMessageContent,
  isJobInbox,
  isLocalMessage,
} from "@shinkai_network/shinkai-message-ts/utils";
import { Placeholder } from "@tiptap/extension-placeholder";
import { EditorContent, useEditor } from "@tiptap/react";
import { StarterKit } from "@tiptap/starter-kit";
import MarkdownPreview from "@uiw/react-markdown-preview";
import { Loader } from "lucide-react";
import { Markdown } from "tiptap-markdown";
import { z } from "zod";

import { useSendMessageToJob } from "../../api/mutations/sendMessageToJob/useSendMessageToJob";
import { useSendMessageToInbox } from "../../api/mutations/sendTextMessage/useSendMessageToInbox";
import { useGetChatConversationWithPagination } from "../../api/queries/getChatConversation/useGetChatConversationWithPagination";
import { Avatar, AvatarFallback, AvatarImage } from "../../components/ui/avatar";
import { Button } from "../../components/ui/button";
import DotsLoader from "../../components/ui/dots-loader";
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
import {
  getMessageFromChat,
  getMessageFromJob,
  groupMessagesByDate,
} from "../../lib/chat-conversation";
import { cn } from "../../lib/utils";
import { useAuth } from "../../store/auth";

const chatSchema = z.object({
  message: z.string(),
});

const ChatConversation = () => {
  const { inboxId: encodedInboxId = "" } = useParams();
  const auth = useAuth((state) => state.auth);
  const chatContainerRef = useRef<HTMLDivElement | null>(null);
  const previousChatHeightRef = useRef<number>(0);
  const fromPreviousMessagesRef = useRef<boolean>(false);

  const inboxId = decodeURIComponent(encodedInboxId);
  console.log({
    inboxId,
    encodedInboxId,
  });
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
    shinkaiIdentity: auth?.shinkai_identity ?? "",
    profile: auth?.profile ?? "",
    my_device_encryption_sk: auth?.my_device_encryption_sk ?? "",
    my_device_identity_sk: auth?.my_device_identity_sk ?? "",
    node_encryption_pk: auth?.node_encryption_pk ?? "",
    profile_encryption_sk: auth?.profile_encryption_sk ?? "",
    profile_identity_sk: auth?.profile_identity_sk ?? "",
  });

  const { mutateAsync: sendMessageToInbox, isLoading: isSendingMessageToInbox } =
    useSendMessageToInbox();
  const { mutateAsync: sendMessageToJob, isLoading: isSendingMessageToJob } =
    useSendMessageToJob();

  const onSubmit = async (data: z.infer<typeof chatSchema>) => {
    if (!auth) return;
    fromPreviousMessagesRef.current = false;
    if (isJobInbox(inboxId)) {
      const sender = `${auth.shinkai_identity}/${auth.profile}`;
      const jobId = extractJobIdFromInbox(inboxId);
      sendMessageToJob({
        jobId,
        message: data.message,
        sender,
        files_inbox: "",
        shinkaiIdentity: auth.shinkai_identity,
        my_device_encryption_sk: auth.my_device_encryption_sk,
        my_device_identity_sk: auth.my_device_identity_sk,
        node_encryption_pk: auth.node_encryption_pk,
        profile_encryption_sk: auth.profile_encryption_sk,
        profile_identity_sk: auth.profile_identity_sk,
      });
    } else {
      const sender = `${auth.shinkai_identity}/${auth.profile}/device/${auth.registration_name}`;
      const receiver = extractReceiverShinkaiName(inboxId, sender);
      sendMessageToInbox({
        sender,
        receiver,
        message: data.message,
        inboxId: inboxId as string,
        my_device_encryption_sk: auth.profile_encryption_sk,
        my_device_identity_sk: auth.profile_identity_sk,
        node_encryption_pk: auth.node_encryption_pk,
        profile_encryption_sk: auth.profile_encryption_sk,
        profile_identity_sk: auth.profile_identity_sk,
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
      <ScrollArea className="h-full px-5" ref={chatContainerRef}>
        {isChatConversationSuccess && (
          <div className="py-2 text-center text-xs">
            {isFetchingPreviousPage || hasPreviousPage ? (
              <Loader className="flex animate-spin justify-center text-white" />
            ) : (
              "All messages has been loaded ✅"
            )}
          </div>
        )}
        <div className="pb-4">
          {isChatConversationLoading &&
            [1, 2, 3, 4].map((index) => (
              <Skeleton className="h-10 w-full rounded-lg" key={index} />
            ))}
          {isChatConversationSuccess &&
            data?.pages.map((group, index) => (
              <Fragment key={index}>
                {Object.entries(groupMessagesByDate(group)).map(([date, messages]) => {
                  return (
                    <div key={date}>
                      <div
                        className={cn(
                          "relative z-10 m-auto flex w-[140px] items-center justify-center rounded-xl border border-slate-800 bg-[#131B23] transition-opacity",
                          true && "sticky top-5"
                        )}
                      >
                        <span className="px-2.5 py-2 text-sm text-foreground">
                          {date}
                        </span>
                      </div>
                      <div className="flex flex-col gap-4">
                        {messages.map((message) => {
                          const isLocal = isLocalMessage(
                            message,
                            auth?.shinkai_identity ?? "",
                            auth?.profile ?? ""
                          );

                          return (
                            <div
                              className={cn(
                                "flex w-[95%] items-start gap-3",
                                isLocal
                                  ? "ml-0 mr-auto flex-row"
                                  : "ml-auto mr-0 flex-row-reverse"
                              )}
                              key={message.external_metadata?.scheduled_time}
                            >
                              <Avatar className="mt-1 h-8 w-8">
                                <AvatarImage
                                  src={
                                    isLocal
                                      ? `https://ui-avatars.com/api/?name=${inboxId}&background=0b1115&color=c7c7c7`
                                      : `https://ui-avatars.com/api/?name=S&background=FF5E5F&color=fff`
                                  }
                                  alt={isLocal ? inboxId : "Shinkai AI"}
                                />
                                <AvatarFallback className="h-8 w-8" />
                              </Avatar>
                              <MarkdownPreview
                                className={cn(
                                  "mt-1 rounded-lg bg-transparent px-2.5 py-3 text-sm text-foreground",
                                  isLocal
                                    ? "rounded-tl-none border border-slate-800"
                                    : "rounded-tr-none border-none bg-[rgba(217,217,217,0.04)]"
                                )}
                                wrapperElement={{
                                  "data-color-mode": "dark",
                                }}
                                source={getMessageContent(message)}
                              />
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  );
                })}
              </Fragment>
            ))}
        </div>
      </ScrollArea>

      <div className="flex flex-col justify-start">
        <div className="relative flex items-start gap-2 bg-app-gradient p-2 pt-3">
          {isLoading ? (
            <DotsLoader className="absolute left-8 top-10 flex items-center justify-center" />
          ) : null}

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
    autofocus: true,
    onUpdate({ editor }) {
      // onChange(editor.getHTML());
      onChange(editor.storage.markdown.getMarkdown());
    },
  });

  useEffect(() => {
    editor?.setEditable(!disabled);
  }, [disabled, editor]);

  useEffect(() => {
    setInitialValue === undefined
      ? editor?.commands.setContent("")
      : editor?.commands.setContent(value);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [setInitialValue]);

  useEffect(() => {
    if (value === "") editor?.commands.setContent("");
  }, [value, editor]);

  return <EditorContent editor={editor} />;
};
