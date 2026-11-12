/** Browser-side session calls. */

import { browserFetch, browserFetchVoid } from "./transport";

export const authApi = {
  login(username: string, password: string): Promise<void> {
    return browserFetchVoid("/api/auth/login", {
      method: "POST",
      body: { username, password },
    });
  },

  logout(): Promise<void> {
    return browserFetchVoid("/api/auth/logout", { method: "POST" });
  },
};
