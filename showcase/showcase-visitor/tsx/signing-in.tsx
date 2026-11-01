// Visitor sign-in, while reception's building service takes the
// sign-in. Fictional brand; every text invented.
import { Column, Screen, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 24;

export function SigningInScreen() {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title">Signing you in</Text>
        <Text>Telling reception you are here. This takes a moment.</Text>
        <Text>Demo: reception's building service is simulated</Text>
      </Column>
    </Screen>
  );
}
