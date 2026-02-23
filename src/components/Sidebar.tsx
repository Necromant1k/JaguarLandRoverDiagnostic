import type { Tab } from "../types";

interface Props {
  activeTab: Tab;
  onTabChange: (tab: Tab) => void;
  connected: boolean;
}

const tabs: { id: Tab; label: string; icon: string }[] = [
  { id: "connect", label: "Connect", icon: "plug" },
  { id: "imc", label: "IMC", icon: "cpu" },
  { id: "bcm", label: "BCM", icon: "shield" },
  { id: "gwm", label: "GWM", icon: "network" },
  { id: "ipc", label: "IPC", icon: "gauge" },
];

const icons: Record<string, string> = {
  plug: "M13 10V3L4 14h7v7l9-11h-7z",
  cpu: "M9 3v2M15 3v2M9 19v2M15 19v2M3 9h2M3 15h2M19 9h2M19 15h2M7 7h10v10H7z",
  shield: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z",
  network: "M9 3H5a2 2 0 00-2 2v4m6-6h10a2 2 0 012 2v4M9 3v18m0 0h10a2 2 0 002-2v-4M9 21H5a2 2 0 01-2-2v-4m0 0h18",
  gauge: "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm.5-13H11v6l5.25 3.15.75-1.23-4.5-2.67V7z",
};

export default function Sidebar({ activeTab, onTabChange, connected }: Props) {
  return (
    <nav className="w-44 bg-bg-secondary border-r border-[#444] flex flex-col py-2 shrink-0">
      {tabs.map(({ id, label, icon }) => {
        const isActive = activeTab === id;
        const disabled = id !== "connect" && !connected;
        return (
          <button
            key={id}
            onClick={() => !disabled && onTabChange(id)}
            disabled={disabled}
            className={`
              flex items-center gap-3 px-4 py-2.5 text-sm text-left transition-all
              ${isActive ? "bg-accent/10 text-accent border-r-2 border-accent" : "text-gray-400 hover:text-gray-200 hover:bg-bg-hover"}
              ${disabled ? "opacity-30 cursor-not-allowed" : "cursor-pointer"}
            `}
          >
            <svg
              className="w-4 h-4 shrink-0"
              fill="none"
              stroke="currentColor"
              strokeWidth={2}
              viewBox="0 0 24 24"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d={icons[icon]} />
            </svg>
            {label}
          </button>
        );
      })}
    </nav>
  );
}
