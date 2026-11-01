// Kiosk reference screen, phase Welcome: no session is open.
// Synthetic and sector-neutral, written for the kiosk reference; not taken
// from any product or customer.
import { Button, Column, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 32;

type WelcomeProps = {
  onStart: () => void;
};

export function WelcomeScreen({ onStart }: WelcomeProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title">Welcome</Text>
        <Text>Ask a question and the service desk will reply.</Text>
        <Text>This kiosk keeps nothing once you finish.</Text>
        <Spacer />
        <Button id="start" onPress={onStart}>
          Start
        </Button>
      </Column>
    </Screen>
  );
}
