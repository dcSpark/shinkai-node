import { Outlet, useNavigate } from "react-router-dom";

("use client");

import * as React from "react";
import { GearIcon, LightningBoltIcon, PersonIcon } from "@radix-ui/react-icons";

import {
  Command,
  CommandGroup,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "../../components/ui/command";
import { Separator } from "../../components/ui/separator";
import { Popover, PopoverContent, PopoverTrigger } from "../../components/ui/popover";
import {
  ADD_AGENT_PATH,
  CREATE_CHAT_PATH,
  CREATE_JOB_PATH,
  SETTINGS_PATH,
} from "../../routes/name";
import { ShinkaiLogo } from "../../components/icons";

export function Footer() {
  const [open, setOpen] = React.useState(false);
  const navigate = useNavigate();

  React.useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen((open) => !open);
      }
    };

    document.addEventListener("keydown", down);
    return () => document.removeEventListener("keydown", down);
  }, []);

  const goToCreateJob = () => {
    navigate(CREATE_JOB_PATH);
  };
  const goToCreateChat = () => {
    navigate(CREATE_CHAT_PATH);
  };
  const goToCreateAgent = () => {
    navigate(ADD_AGENT_PATH);
  };

  const goToProfile = () => {
    navigate(SETTINGS_PATH);
  };
  const goToSettings = () => {
    navigate(SETTINGS_PATH);
  };

  const handleCommandCardKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.metaKey || e.ctrlKey) {
      switch (e.key) {
        case "1":
          goToCreateAgent();
          break;
        case "2":
          goToCreateChat();
          break;
        case "3":
          goToCreateAgent();
          break;
        case "4":
          goToProfile();
          break;
        case "5":
          goToSettings();
          break;
        default:
          break;
      }
    }
  };

  return (
    <div className="flex items-center justify-start text-sm text-muted-foreground bg-app-gradient py-2 px-2">
      <ShinkaiLogo className="w-4 h-4 mr-2 text-primary-600" />
      <Popover open={open} onOpenChange={setOpen} modal>
        <PopoverTrigger
          className="rounded-lg p-1 px-2 transition-colors duration-150 hover:bg-gray-800/60"
          onClick={() => setOpen(true)}
          aria-expanded={open}
        >
          <span className="">
            Actions{" "}
            <kbd className="pointer-events-none inline-flex h-5 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-[10px] font-medium text-muted-foreground opacity-100">
              <span className="text-xs">⌘</span>K
            </kbd>
          </span>
        </PopoverTrigger>
        <PopoverContent
          side="top"
          align="start"
          className="bg-black border-0 bg-gradient-to-r from-[#19242D] to-[#19242D]/90 rounded-md shadow-xl p-4"
          sideOffset={2}
          alignOffset={0}
          asChild
        >
          <Command
            onKeyDown={handleCommandCardKeyDown}
            className="p-0 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-gray-500 [&_[cmdk-group]:not([hidden])_~[cmdk-group]]:pt-0 [&_[cmdk-group]]:px-2 [&_[cmdk-input-wrapper]_svg]:h-5 [&_[cmdk-input-wrapper]_svg]:w-5 [&_[cmdk-input]]:h-12 [&_[cmdk-item]]:px-2 [&_[cmdk-item]]:py-3 [&_[cmdk-item]_svg]:h-3 [&_[cmdk-item]_svg]:w-3 dark:[&_[cmdk-group-heading]]:text-gray-400"
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
    <div className="relative flex flex-col h-full bg-cover bg-app-gradient text-white">
      <div
        data-tauri-drag-region
        className="flex select-none cursor-default justify-center items-center h-8 text-xs"
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
