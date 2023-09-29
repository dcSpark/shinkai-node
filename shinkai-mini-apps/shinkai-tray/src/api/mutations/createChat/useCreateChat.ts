import { useMutation } from "@tanstack/react-query";
import type { UseMutationOptions } from "@tanstack/react-query";
import { createChat } from ".";
import { CreateChatInput, CreateChatOutput } from "./types";
import { FunctionKey, queryClient } from "../../constants";

type Options = UseMutationOptions<CreateChatOutput, Error, CreateChatInput>;

export const useCreateChat = (options?: Options) => {
  return useMutation({
    mutationFn: createChat,
    onSuccess: (...onSuccessParams) => {
      queryClient.invalidateQueries([FunctionKey.GET_INBOXES]);
      if (options?.onSuccess) {
        options.onSuccess(...onSuccessParams);
      }
    },
  });
};
