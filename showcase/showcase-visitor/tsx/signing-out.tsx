// Visitor sign-in, signing out: nothing to do here, because the
// kiosk keeps no list of visitors - hand the pass back to reception.
// Fixed text, shown outside any session. Fictional brand; every text
// invented.
import { Button, Column, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 24;

type SigningOutProps = {
  onBack: () => void;
};

export function SigningOutScreen({ onBack }: SigningOutProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title">Signing out</Text>
        <Text>Hand your pass to reception, or leave it in the box by the door.</Text>
        <Text>This kiosk keeps no list of visitors, so there is nothing to sign out here.</Text>
        <Spacer />
        <Button id="back" onPress={onBack}>
          Back
        </Button>
      </Column>
    </Screen>
  );
}
