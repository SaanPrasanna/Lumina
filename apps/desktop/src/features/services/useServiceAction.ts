import { useMutation, useQueryClient } from "@tanstack/react-query";

/** Runs a service action and refreshes everything that shows service state. */
export function useServiceAction() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: ["services"] }),
        queryClient.invalidateQueries({ queryKey: ["settings"] }),
      ]),
  });
}
