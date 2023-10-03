import * as React from "react";
import { Outlet, useNavigate } from "react-router-dom";

import { GearIcon, LightningBoltIcon, PersonIcon } from "@radix-ui/react-icons";

import { ShinkaiLogo } from "../../components/icons";
import {
  Command,
  CommandGroup,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "../../components/ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "../../components/ui/popover";
import { Separator } from "../../components/ui/separator";
import {
  ADD_AGENT_PATH,
  CREATE_CHAT_PATH,
  CREATE_JOB_PATH,
  ONBOARDING_PATH,
  SETTINGS_PATH,
} from "../../routes/name";
import { useAuth } from "../../store/auth-context";

export function Footer() {
  const [open, setOpen] = React.useState(false);
  const navigate = useNavigate();
  const { setSetupData } = useAuth();

  React.useEffect(() => {
    const down = (event: KeyboardEvent) => {
      if (event.key === "k" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        setOpen((open) => !open);
      }
    };

    document.addEventListener("keydown", down);
    return () => document.removeEventListener("keydown", down);
  }, []);

  const goToCreateJob = () => {
    navigate(CREATE_JOB_PATH);
    setOpen(false);
  };
  const goToCreateChat = () => {
    navigate(CREATE_CHAT_PATH);
    setOpen(false);
  };
  const goToCreateAgent = () => {
    navigate(ADD_AGENT_PATH);
    setOpen(false);
  };

  const goToProfile = () => {
    navigate(SETTINGS_PATH);
    setOpen(false);
  };
  const goToSettings = () => {
    navigate(SETTINGS_PATH);
    setOpen(false);
  };
  const handleLogout = () => {
    setSetupData(null);
    navigate(ONBOARDING_PATH);
  };

  const handleCommandCardKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.metaKey || event.ctrlKey) {
      switch (event.key) {
        case "1": {
          goToCreateJob();
          break;
        }
        case "2": {
          goToCreateChat();
          break;
        }
        case "3": {
          goToCreateAgent();
          break;
        }
        case "4": {
          goToProfile();
          break;
        }
        case "5": {
          goToSettings();
          break;
        }
        case "6": {
          handleLogout();
          break;
        }
        default: {
          break;
        }
      }
    }
  };

  return (
    <div className="flex items-center justify-start bg-app-gradient p-2 pl-4 text-sm text-muted-foreground">
      <ShinkaiLogo className="mr-2 h-4 w-4 text-primary-600" />
      <Popover onOpenChange={setOpen} open={open} modal>
        <PopoverTrigger
          aria-expanded={open}
          className="rounded-lg p-1 px-2 transition-colors duration-150 hover:bg-gray-800/60"
          onClick={() => setOpen(true)}
        >
          <span className="text-xs">
            Actions
            <kbd className="bg-muted pointer-events-none ml-2 inline-flex h-5 select-none items-center gap-1 rounded border px-1.5 font-mono text-[10px] font-medium text-muted-foreground opacity-100">
              <span className="text-xs">⌘</span>K
            </kbd>
          </span>
        </PopoverTrigger>
        <PopoverContent
          align="start"
          alignOffset={-25}
          className="rounded-md border-0 bg-black bg-gradient-to-r from-[#19242D] to-[#19242D]/90 p-4 shadow-xl"
          side="top"
          sideOffset={2}
          asChild
        >
          <Command
            className="p-0 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-gray-500 dark:[&_[cmdk-group-heading]]:text-gray-400 [&_[cmdk-group]:not([hidden])_~[cmdk-group]]:pt-0 [&_[cmdk-group]]:px-2 [&_[cmdk-input-wrapper]_svg]:h-5 [&_[cmdk-input-wrapper]_svg]:w-5 [&_[cmdk-input]]:h-12 [&_[cmdk-item]]:px-2 [&_[cmdk-item]]:py-3 [&_[cmdk-item]_svg]:h-3 [&_[cmdk-item]_svg]:w-3"
            onKeyDown={handleCommandCardKeyDown}
          >
            <CommandList className="p-0 pt-2">
              <CommandGroup heading="Actions">
                <CommandItem onSelect={goToCreateJob}>
                  <LightningBoltIcon className="mr-2" />
                  <span>Create Job</span>
                  <CommandShortcut>⌘1</CommandShortcut>
                </CommandItem>
                <CommandItem onSelect={goToCreateChat}>
                  <LightningBoltIcon className="mr-2" />
                  <span>Create Chat</span>
                  <CommandShortcut>⌘2</CommandShortcut>
                </CommandItem>
                <CommandItem onSelect={goToCreateAgent}>
                  <LightningBoltIcon className="mr-2" />
                  <span>Add Agent</span>
                  <CommandShortcut>⌘3</CommandShortcut>
                </CommandItem>
              </CommandGroup>
              <CommandSeparator />
              <CommandGroup heading="General">
                <CommandItem onSelect={goToProfile}>
                  <PersonIcon className="mr-2 h-4 w-4" />
                  <span>Profile</span>
                  <CommandShortcut>⌘4</CommandShortcut>
                </CommandItem>
                <CommandItem onSelect={goToSettings}>
                  <GearIcon className="mr-2 h-4 w-4" />
                  <span>Settings</span>
                  <CommandShortcut>⌘5</CommandShortcut>
                </CommandItem>
                <CommandItem onSelect={handleLogout}>
                  <GearIcon className="mr-2 h-4 w-4" />
                  <span>Logout</span>
                  <CommandShortcut>⌘6</CommandShortcut>
                </CommandItem>
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
    </div>
  );
}

const MainLayout = () => {
  return (
    <div className="relative flex h-full flex-col bg-app-gradient bg-cover text-white">
      <div
        className="flex h-8 shrink-0 cursor-default select-none items-center justify-center text-xs"
        data-tauri-drag-region
      >
        Custom Topbar (draggable)
      </div>
      <Separator />
      <div className="flex-auto overflow-auto">
        <Outlet />
      </div>
      <Footer />
    </div>
  );
};
export default MainLayout;
