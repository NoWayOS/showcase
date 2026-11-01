// Pay at the pump, the welcome screen: the screen beside a
// fictional fuel station's pumps. The driver chooses here and pays on the
// card reader: this screen never sees a card. Fictional brand; every text
// invented.
import { Button, Column, Notice, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 24;
const TIGHT = 12;

type WelcomeProps = {
  onStart: () => void;
};

export function WelcomeScreen({ onStart }: WelcomeProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text role="caption">Tarnwick Fuel - pumps 1 to 4</Text>
        <Text id="title" role="title">
          Pay at the pump
        </Text>
        <Text>Choose your pump, fuel and amount here, then pay on the card reader.</Text>
        <Button id="start" onPress={onStart}>
          Start
        </Button>
        <Notice tone="success" padding={TIGHT}>
          <Text weight="bold">This screen never sees your card.</Text>
        </Notice>
        <Spacer />
        <Text role="caption">Demo: the card reader, the bank and the pumps are simulated</Text>
      </Column>
    </Screen>
  );
}
