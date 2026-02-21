import type { Tab } from "../types";

interface Props {
  activeTab: Tab;
  onTabChange: (tab: Tab) => void;
  connected: boolean;
}

const tabs: { id: Tab; label: string; icon: string }[] = [
  { id: "connect", label: "Connect", icon: "plug" },
  { id: "vehicle", label: "Vehicle", icon: "car" },
  { id: "ssh", label: "SSH", icon: "terminal" },
  { id: "imc", label: "IMC", icon: "cpu" },
];

const icons: Record<string, string> = {
  plug: "M13 10V3L4 14h7v7l9-11h-7z",
  car: "M5 11l1.5-4.5h11L19 11M3 15h18v-4l-2-4H5L3 11v4zm3 3a1.5 1.5 0 100-3 1.5 1.5 0 000 3zm12 0a1.5 1.5 0 100-3 1.5 1.5 0 000 3z",
  terminal: "M4 17l6-6-6-6M12 19h8",
  cpu: "M9 3v2M15 3v2M9 19v2M15 19v2M3 9h2M3 15h2M19 9h2M19 15h2M7 7h10v10H7z",
};

export default function Sidebar({ activeTab, onTabChange, connected }: Props) {
  return (
    <nav className="w-44 bg-bg-secondary border-r border-gray-700/50 flex flex-col py-2 shrink-0">
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
