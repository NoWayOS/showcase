// The bar above every Service desk screen in the showcase, after
// the approved mockup (ServiceDesk): Home, which wipes the example
// and shows the launcher again, the example's name, and the demo line.
// The showcase builds it above each kiosk reference screen, its ids moved
// past the screen's own. A header bar, in the example's brand
// colour.
import { Button, HeaderBar, Icon, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 16;
const EDGE = 8;

type ServiceDeskBarProps = {
  onHome: () => void;
};

export function ServiceDeskBarScreen({ onHome }: ServiceDeskBarProps) {
  return (
    <Screen>
      <HeaderBar gap={GAP} padding={EDGE}>
        <Button id="home" variant="secondary" onPress={onHome}>
          Home
        </Button>
        <Icon name="message-square" />
        <Text role="heading">Service desk</Text>
        <Spacer />
        <Text>Demo: simulated, nothing kept</Text>
      </HeaderBar>
    </Screen>
  );
}
