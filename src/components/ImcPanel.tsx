import EcuInfoSection from "./EcuInfoSection";
import RoutinesPanel from "./RoutinesPanel";

interface Props {
  connected: boolean;
}

export default function ImcPanel({ connected }: Props) {
  return (
    <div className="space-y-6 max-w-2xl">
      <h2 className="text-lg font-bold text-accent">IMC</h2>
      <EcuInfoSection ecuId="imc" connected={connected} />
      <RoutinesPanel connected={connected} />
    </div>
  );
}
