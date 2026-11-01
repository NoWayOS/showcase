// Visitor sign-in, step 4: the site's rules for visitors, which
// the visitor agrees to follow. Not a fire roll, no evacuation steps.
// Fictional brand; every text invented.
import { Button, Checkbox, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 24;

type SiteRulesProps = {
  rulesError: string;
  onBack: () => void;
  onCancel: () => void;
  onNext: () => void;
};

export function SiteRulesStep(props: SiteRulesProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading">Site rules</Text>
        <Text>Wear your pass where others can see it.</Text>
        <Text>Stay with the person you are visiting.</Text>
        <Text>Ask at reception if you need help.</Text>
        <Checkbox id="rules" label="I will follow the site rules" />
        {props.rulesError && <Text>{props.rulesError}</Text>}
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onBack}>Back</Button>
          <Button onPress={props.onCancel}>Cancel</Button>
          <Spacer />
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
