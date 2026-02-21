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
];

const icons: Record<string, string> = {
  plug: "M13 10V3L4 14h7v7l9-11h-7z",
  cpu: "M9 3v2M15 3v2M9 19v2M15 19v2M3 9h2M3 15h2M19 9h2M19 15h2M7 7h10v10H7z",
  shield: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z",
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
