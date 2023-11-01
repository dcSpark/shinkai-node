import type { UseMutationOptions } from "@tanstack/react-query";

import { useMutation } from "@tanstack/react-query";

import type { UpdateInboxNameOutput, UpdateInboxNamebInput } from "./types";

import { updateInboxName } from ".";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Options = UseMutationOptions<UpdateInboxNameOutput, Error, UpdateInboxNamebInput>;

export const useUpdateInboxName = (options?: Options) => {
  return useMutation({
    mutationFn: updateInboxName,
    ...options,
  });
};
