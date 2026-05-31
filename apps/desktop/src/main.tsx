import { Toast } from "@heroui/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import React from "react";
import ReactDOM from "react-dom/client";
import { router } from "./router";
import "./styles.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 2 * 60 * 1000,   // 2 min — don't refetch if fresh
      gcTime: 30 * 60 * 1000,     // 30 min — keep in cache even after unmount
      refetchOnWindowFocus: false, // don't spam API on tab switch
      retry: 1,
    },
  },
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      <Toast.Provider placement="top" />
    </QueryClientProvider>
  </React.StrictMode>,
);
