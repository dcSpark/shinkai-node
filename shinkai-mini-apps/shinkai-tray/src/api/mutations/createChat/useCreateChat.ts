import { useMutation } from "@tanstack/react-query";
import type { UseMutationOptions } from "@tanstack/react-query";
import { createChat } from ".";
import { CreateChatInput, CreateChatOutput } from "./types";

type Options = UseMutationOptions<CreateChatOutput, Error, CreateChatInput>;

export const useCreateChat = (options?: Options) => {
  return useMutation({
    mutationFn: createChat,
    ...options,
  });
};
