import { cookies, headers } from "next/headers";
import { getRequestConfig } from "next-intl/server";

import { defaultLocale, isLocale, localeCookie, negotiate } from "./config";

export default getRequestConfig(async () => {
  const stored = (await cookies()).get(localeCookie)?.value;
  const locale = isLocale(stored)
    ? stored
    : (negotiate((await headers()).get("accept-language")) ?? defaultLocale);
  return {
    locale,
    messages: (await import(`../messages/${locale}.json`)).default,
  };
});
