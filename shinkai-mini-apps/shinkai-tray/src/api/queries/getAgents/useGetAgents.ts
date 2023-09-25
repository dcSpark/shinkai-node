import { useQuery } from "@tanstack/react-query";
import { getAgents } from ".";
import { GetAgents } from "./types";
import { FunctionKey } from "../../constants";

export const useAgents = (input: GetAgents) => {
  const response = useQuery({
    queryKey: [FunctionKey.GET_AGENTS, input],
    queryFn: () => getAgents(input),
  });
  return { ...response, agents: response.data };
};
