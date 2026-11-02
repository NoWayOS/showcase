// The Screen Compiler gallery, 7 of 11: a checkbox, a radio group and a list picker. The application reads
// and sets each by its id; what is chosen here is wiped as the screen
// closes.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Checkbox, Column, HeaderBar, RadioGroup, Row, Screen, Select, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
};

export function GalleryChoicesScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Choices
          </Text>
          <Spacer />
          <Text>
            7 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Row gap={EDGE}>
          <Column gap={4}>
            <Checkbox id="remind" label="Remind me" />
            <Text role="caption">{'<Checkbox id="remind" label="Remind me" />'}</Text>
            <RadioGroup id="when" options={["Morning", "Afternoon", "Evening"]} />
            <Text role="caption">{'<RadioGroup id="when" options={[...]} />'}</Text>
          </Column>
          <Column gap={4}>
            <Select
              id="colour"
              placeholder="Choose a colour"
              rows={3}
              options={["Black", "Blue", "Green", "Grey", "Red", "White", "Yellow"]}
            />
            <Text role="caption">{'<Select id="colour" rows={3} options={[...]} />: up to 64'}</Text>
          </Column>
        </Row>
      </Column>
    </Screen>
  );
}
