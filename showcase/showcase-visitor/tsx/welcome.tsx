// Visitor sign-in, the welcome screen: a fictional research
// centre's reception, after the approved mockup (Visitor). Sign in,
// or learn how to sign out; what the screen keeps, wrapped onto up to
// three lines. Fictional brand; every text invented.
import { Button, Column, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 10;
const EDGE = 24;

type WelcomeProps = {
  onStart: () => void;
  onSigningOut: () => void;
};

export function WelcomeScreen({ onStart, onSigningOut }: WelcomeProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text>Orrinvale Research Centre - Main reception</Text>
        <Text id="title">Welcome. Are you visiting today?</Text>
        <Text>Sign in so reception knows you are here.</Text>
        <Button id="start" onPress={onStart}>
          Sign in as a visitor
        </Button>
        <Button id="signing-out" onPress={onSigningOut}>
          Signing out
        </Button>
        <Text lines={3}>
          {"This screen forgets you when you finish. Your pass shows a number, never your name, and nobody after you sees what you typed."}
        </Text>
        <Spacer />
        <Text>Demo: reception and pass printing are simulated</Text>
      </Column>
    </Screen>
  );
}
