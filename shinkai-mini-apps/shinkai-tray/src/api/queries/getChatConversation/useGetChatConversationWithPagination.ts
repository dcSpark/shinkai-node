import { useInfiniteQuery } from "@tanstack/react-query";
import { getChatConversation } from ".";
import { GetChatConversationInput } from "./types";
import { FunctionKey } from "../../constants";

export const CONVERSATION_PAGINATION_LIMIT = 6;

export const useGetChatConversationWithPagination = (input: GetChatConversationInput) => {
  const response = useInfiniteQuery({
    queryKey: [FunctionKey.GET_CHAT_CONVERSATION_PAGINATION, input],
    queryFn: ({ pageParam }) =>
      getChatConversation({
        ...input,
        lastKey: pageParam?.lastKey ?? undefined,
      }),
    getPreviousPageParam: (firstPage) => {
      return firstPage?.length === CONVERSATION_PAGINATION_LIMIT;
    },
  });
  return response;
};
