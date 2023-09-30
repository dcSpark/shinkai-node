import { useMutation } from "@tanstack/react-query";
import type { UseMutationOptions } from "@tanstack/react-query";
import { SetupDataArgs, submitRegistration } from ".";
import type { APIUseRegistrationCodeSuccessResponse } from "@shinkai_network/shinkai-message-ts/models";

type Data = {
  success: boolean;
  data?: APIUseRegistrationCodeSuccessResponse | undefined;
};
type Options = UseMutationOptions<Data, Error, SetupDataArgs>;

export const useSubmitRegistration = (options?: Options) => {
  return useMutation({
    mutationFn: submitRegistration,
    ...options,
  });
};
