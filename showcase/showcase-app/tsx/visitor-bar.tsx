// The bar above every Visitor sign-in screen in the showcase, as
// the Service desk's: Home, which wipes the example and shows the
// launcher again, the example's name, and what is simulated. The showcase
// builds it above each visitor screen, its ids moved past the screen's own.
// A header bar, in the example's brand colour, its icon.
import { Button, HeaderBar, Icon, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 16;
const EDGE = 8;

type VisitorBarProps = {
  onHome: () => void;
};

export function VisitorBarScreen({ onHome }: VisitorBarProps) {
  return (
    <Screen>
      <HeaderBar gap={GAP} padding={EDGE}>
        <Button id="home" variant="secondary" onPress={onHome}>
          Home
        </Button>
        <Icon name="user-check" />
        <Text role="heading">Visitor sign-in</Text>
        <Spacer />
        <Text>Demo: reception simulated, nothing kept</Text>
      </HeaderBar>
    </Screen>
  );
}
