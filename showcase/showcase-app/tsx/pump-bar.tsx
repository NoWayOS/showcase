// The bar above every pay at the pump screen in the showcase, as the
// Service desk's: Home, which wipes the example and shows the
// launcher again, the example's name, and what is simulated. The showcase
// builds it above each pump screen, its ids moved past the screen's own.
// A header bar, in the example's brand colour, its icon.
import { Button, HeaderBar, Icon, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 16;
const EDGE = 8;

type PumpBarProps = {
  onHome: () => void;
};

export function PumpBarScreen({ onHome }: PumpBarProps) {
  return (
    <Screen>
      <HeaderBar gap={GAP} padding={EDGE}>
        <Button id="home" variant="secondary" onPress={onHome}>
          Home
        </Button>
        <Icon name="fuel" />
        <Text role="heading">Pay at the pump</Text>
        <Spacer />
        <Text>Demo: card reader and pumps simulated, nothing kept</Text>
      </HeaderBar>
    </Screen>
  );
}
