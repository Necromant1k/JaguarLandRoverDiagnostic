import EcuInfoSection from "./EcuInfoSection";

interface Props {
  connected: boolean;
}

export default function BcmPanel({ connected }: Props) {
  return (
    <div className="space-y-6 max-w-2xl">
      <h2 className="text-lg font-bold text-accent">BCM</h2>
      <EcuInfoSection ecuId="bcm" connected={connected} />
      <div className="card text-center text-[#858585] text-sm py-8">
        No routines defined for BCM
      </div>
    </div>
  );
}
