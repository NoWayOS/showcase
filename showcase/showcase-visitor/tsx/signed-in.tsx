// Visitor sign-in, signed in: the pass number the building service
// issued - a number, never the visitor's name, every answer already wiped -
// and the screen clears itself, counting down.
// Fictional brand; every text invented.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 10;
const EDGE = 24;

type SignedInProps = {
  pass: string;
  secondsLeft: number;
  onDone: () => void;
};

export function SignedInScreen({ pass, secondsLeft, onDone }: SignedInProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title">You are signed in</Text>
        <Row gap={GAP}>
          <Text>Your pass number:</Text>
          <Text>{pass}</Text>
        </Row>
        <Text>Collect your pass from reception. Your answers are already cleared.</Text>
        <Text>Demo: reception and pass printing are simulated</Text>
        <Row gap={GAP}>
          <Text>Seconds left:</Text>
          <Text>{secondsLeft}</Text>
        </Row>
        <Spacer />
        <Button id="done" onPress={onDone}>
          Done
        </Button>
      </Column>
    </Screen>
  );
}
