import type { ShinkaiMessage } from "@shinkai_network/shinkai-message-ts/models";

import { Fragment, useCallback, useEffect, useMemo, useRef } from "react";
import { useDropzone } from "react-dropzone";
import { useForm } from "react-hook-form";
import { useParams } from "react-router-dom";

import { zodResolver } from "@hookform/resolvers/zod";
import { PaperPlaneIcon } from "@radix-ui/react-icons";
import { MessageSchemaType } from "@shinkai_network/shinkai-message-ts/models";
import {
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
import { ImagePlusIcon, Loader, XCircleIcon } from "lucide-react";
import { Markdown } from "tiptap-markdown";
import { z } from "zod";

import { useSendMessageToJob } from "../../api/mutations/sendMessageToJob/useSendMessageToJob";
import { useSendMessageWithFilesToInbox } from "../../api/mutations/sendMesssageWithFilesToInbox/useSendMessageWithFilesToInbox";
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
import { formatDate, groupMessagesByDate } from "../../lib/chat-conversation";
import { cn } from "../../lib/utils";
import { useAuth } from "../../store/auth";

export const getMessageFilesInbox = (message: ShinkaiMessage): string | undefined => {
  // unnencrypted content
  if (
    message.body &&
    "unencrypted" in message.body &&
    "unencrypted" in message.body.unencrypted.message_data
  ) {
    const isJobMessage =
      message.body.unencrypted.message_data.unencrypted.message_content_schema ===
      MessageSchemaType.JobMessageSchema;
    // job message
    if (isJobMessage) {
      try {
        const parsedMessage = JSON.parse(
          message.body.unencrypted.message_data.unencrypted.message_raw_content
        );
        return parsedMessage?.files_inbox;
      } catch (error) {
        console.log("error parsing message raw content", error);
      }
    }
  }
  return undefined;
};

const chatSchema = z.object({
  message: z.string(),
  file: z.any().optional(),
});

const ChatConversation = () => {
  const { inboxId: encodedInboxId = "" } = useParams();
  const auth = useAuth((state) => state.auth);
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

  const { getRootProps: getRootFileProps, getInputProps: getInputFileProps } =
    useDropzone({
      multiple: false,
      onDrop: (acceptedFiles) => {
        const file = acceptedFiles[0];
        const reader = new FileReader();
        reader.addEventListener("abort", () => console.log("file reading was aborted"));
        reader.addEventListener("load", (event: ProgressEvent<FileReader>) => {
          const binaryUrl = event.target?.result;
          const image = new Image();
          image.addEventListener("load", function () {
            const imageInfo = Object.assign(file, {
              preview: URL.createObjectURL(file),
            });
            chatForm.setValue("file", imageInfo, { shouldValidate: true });
          });
          image.src = binaryUrl as string;
        });
        reader.readAsDataURL(file);
      },
    });

  const { file } = chatForm.watch();

  const {
    data,
    fetchPreviousPage,
    hasPreviousPage,
    isPending: isChatConversationLoading,
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

  const { mutateAsync: sendMessageToInbox, isPending: isSendingMessageToInbox } =
    useSendMessageToInbox();
  const { mutateAsync: sendMessageToJob, isPending: isSendingMessageToJob } =
    useSendMessageToJob();
  const {
    mutateAsync: sendTextMessageWithFilesForInbox,
    isPending: isSendingTextMessageWithFilesForInbox,
  } = useSendMessageWithFilesToInbox();

  const onSubmit = async (data: z.infer<typeof chatSchema>) => {
    if (!auth) return;
    fromPreviousMessagesRef.current = false;

    if (file) {
      await sendTextMessageWithFilesForInbox({
        sender: auth.shinkai_identity,
        senderSubidentity: auth.profile,
        receiver: auth.shinkai_identity,
        message: data.message,
        inboxId: inboxId,
        file: file,
        my_device_encryption_sk: auth.my_device_encryption_sk,
        my_device_identity_sk: auth.my_device_identity_sk,
        node_encryption_pk: auth.node_encryption_pk,
        profile_encryption_sk: auth.profile_encryption_sk,
        profile_identity_sk: auth.profile_identity_sk,
      });
      chatForm.reset();
      return;
    }

    if (isJobInbox(inboxId)) {
      const jobId = extractJobIdFromInbox(inboxId);
      sendMessageToJob({
        jobId: jobId,
        message: data.message,
        files_inbox: "",
        shinkaiIdentity: auth.shinkai_identity,
        profile: auth.profile,
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
        sender: auth.shinkai_identity,
        sender_subidentity: `${auth.profile}/device/${auth.registration_name}`,
        receiver,
        message: data.message,
        inboxId: inboxId,
        my_device_encryption_sk: auth.my_device_encryption_sk,
        my_device_identity_sk: auth.my_device_identity_sk,
        node_encryption_pk: auth.node_encryption_pk,
        profile_encryption_sk: auth.profile_encryption_sk,
        profile_identity_sk: auth.profile_identity_sk,
      });
    }
    chatForm.reset();
  };

  const isLoading = useMemo(() => {
    if (
      isSendingMessageToJob ||
      isSendingMessageToInbox ||
      isSendingTextMessageWithFilesForInbox
    )
      return true;
    const lastMessage = data?.pages?.at(-1)?.at(-1);
    if (!lastMessage) return false;
    const isLocal = isLocalMessage(
      lastMessage,
      auth?.shinkai_identity ?? "",
      auth?.profile ?? ""
    );
    if (isJobInbox(inboxId) && isLocal) return true;
    return false;
  }, [
    isSendingMessageToJob,
    isSendingMessageToInbox,
    isSendingTextMessageWithFilesForInbox,
    data?.pages,
    auth?.shinkai_identity,
    auth?.profile,
    inboxId,
  ]);

  const fetchPreviousMessages = useCallback(async () => {
    fromPreviousMessagesRef.current = true;
    await fetchPreviousPage();
  }, [fetchPreviousPage]);

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

  useEffect(() => {
    scrollToBottom();
  }, [isChatConversationSuccess]);

  return (
    <div className="flex flex-1 flex-col pt-2">
      <ScrollArea className="h-full px-5" ref={chatContainerRef}>
        {isChatConversationSuccess && (
          <div className="py-2 text-center text-xs">
            {isFetchingPreviousPage ? (
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
                        <span className="px-2.5 py-2 text-sm capitalize text-foreground">
                          {formatDate(new Date(date))}
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
                                  "mt-1 break-all rounded-lg bg-transparent px-2.5 py-3 text-sm text-foreground",
                                  isLocal
                                    ? "rounded-tl-none border border-slate-800"
                                    : "rounded-tr-none border-none bg-[rgba(217,217,217,0.04)]"
                                )}
                                source={`${
                                  getMessageFilesInbox(message)
                                    ? `<svg xmlns="http://www.w3.org/2000/svg" className="mb-2" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-file-text"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/><line x1="16" x2="8" y1="13" y2="13"/><line x1="16" x2="8" y1="17" y2="17"/><line x1="10" x2="8" y1="9" y2="9"/></svg>`
                                    : ""
                                } ${getMessageContent(message)}`}
                                wrapperElement={{
                                  "data-color-mode": "dark",
                                }}
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
            <div
              {...getRootFileProps({
                className: cn(
                  "dropzone group relative relative flex h-16 w-16 flex-shrink-0 cursor-pointer items-center justify-center rounded border-2 border-dashed border-slate-500 border-slate-500 transition-colors hover:border-white",
                  file && "border-0",
                  isLoading && "hidden"
                ),
              })}
            >
              {!file && (
                <ImagePlusIcon className="stroke-slate-500 transition-colors group-hover:stroke-white" />
              )}
              <input
                {...chatForm.register("file")}
                {...getInputFileProps({
                  onChange: chatForm.register("file").onChange,
                })}
              />
              <div>
                {file && (
                  <img
                    alt=""
                    className="absolute left-0 top-0 h-full w-full object-cover"
                    src={file.preview}
                  />
                )}
              </div>
              {file != null && (
                <button
                  className={cn(
                    "absolute -right-1 -top-1 cursor-pointer rounded-full bg-slate-700 hover:bg-slate-900",
                    file ? "block" : "hidden"
                  )}
                  onClick={(event) => {
                    event.stopPropagation();
                    chatForm.setValue("file", undefined, { shouldValidate: true });
                  }}
                >
                  <XCircleIcon className="h-6 w-6" />
                </button>
              )}
            </div>

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
