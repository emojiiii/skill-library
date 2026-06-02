import { useQuery } from "@tanstack/react-query";
import { readSubscriptions } from "../lib/skill-library";
import { SubscriptionsPage } from "../pages/SubscriptionsPage";

export function SubscriptionsRoute() {
  const subscriptions = useQuery({
    queryKey: ["subscriptions"],
    queryFn: readSubscriptions,
    staleTime: 60 * 1000,
  });

  return <SubscriptionsPage subscriptions={subscriptions.data?.subscriptions ?? []} />;
}
