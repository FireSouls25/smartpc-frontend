// Tiny hash router: #/login, #/register, #/ (home). No dependency.
export type RouteName = "home" | "login" | "register";

function parse(): RouteName {
  if (typeof window === "undefined") return "home";
  const h = window.location.hash.replace(/^#\/?/, "");
  if (h === "login") return "login";
  if (h === "register") return "register";
  return "home";
}

let name = $state<RouteName>(parse());

if (typeof window !== "undefined") {
  window.addEventListener("hashchange", () => {
    name = parse();
  });
}

export const route = {
  get name(): RouteName {
    return name;
  },
};

export function navigate(to: "" | "login" | "register"): void {
  window.location.hash = "#/" + to;
}
