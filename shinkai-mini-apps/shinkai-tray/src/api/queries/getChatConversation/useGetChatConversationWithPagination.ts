import { calculateMessageHash } from "@shinkai_network/shinkai-message-ts/utils";
import { InfiniteData, useInfiniteQuery } from "@tanstack/react-query";

import { getChatConversation } from ".";
import { FunctionKey } from "../../constants";
import { GetChatConversationInput, GetChatConversationOutput } from "./types";

export const CONVERSATION_PAGINATION_LIMIT = 10;
export const CONVERSATION_PAGINATION_REFETCH = 5000;

export const useGetChatConversationWithPagination = (input: GetChatConversationInput) => {
  const response = useInfiniteQuery<
    GetChatConversationOutput,
    Error,
    InfiniteData<GetChatConversationOutput>,
    [string, GetChatConversationInput],
    { lastKey: string | null }
  >({
    queryKey: [FunctionKey.GET_CHAT_CONVERSATION_PAGINATION, input],
    queryFn: ({ pageParam }) =>
      getChatConversation({
        ...input,
        lastKey: pageParam?.lastKey ?? undefined,
        count: CONVERSATION_PAGINATION_LIMIT,
      }),
    getPreviousPageParam: (_, pages) => {
      const firstMessage = pages?.[0]?.[0];
      if (!firstMessage) return;
      const timeKey = firstMessage?.external_metadata?.scheduled_time;
      const hashKey = calculateMessageHash(firstMessage);
      const firstMessageKey = `${timeKey}:::${hashKey}`;
      return { lastKey: firstMessageKey };
    },
    refetchInterval: CONVERSATION_PAGINATION_REFETCH,
    initialPageParam: { lastKey: null },
    getNextPageParam: () => {
      return { lastKey: null };
    },
  });
  return response;
};
